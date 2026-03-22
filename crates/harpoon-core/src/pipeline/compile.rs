use crate::types::filter::Filter;
use crate::types::pipeline::*;
use crate::types::rule::ExporterConfig;

use super::validate;

/// Execution strategy determined at compile time.
#[derive(Debug)]
pub enum ExecutionPlan {
    /// Tier 0: Source → Forward only. Zero-copy bidirectional.
    FastForward(FastForwardPlan),

    /// Tier 1: Linear chain. Current filter path.
    Linear(LinearPlan),

    /// Tier 2: DAG with branching.
    Dag(DagPlan),
}

impl ExecutionPlan {
    pub fn name(&self) -> &str {
        match self {
            Self::FastForward(p) => &p.pipeline_name,
            Self::Linear(p) => &p.pipeline_name,
            Self::Dag(p) => &p.pipeline_name,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::FastForward(p) => &p.pipeline_id,
            Self::Linear(p) => &p.pipeline_id,
            Self::Dag(p) => &p.pipeline_id,
        }
    }

    pub fn tier(&self) -> &'static str {
        match self {
            Self::FastForward(_) => "fast_forward",
            Self::Linear(_) => "linear",
            Self::Dag(_) => "dag",
        }
    }
}

#[derive(Debug)]
pub struct FastForwardPlan {
    pub pipeline_id: String,
    pub pipeline_name: String,
    pub source: SourceConfig,
    pub forward: ForwardConfig,
}

#[derive(Debug)]
pub struct LinearPlan {
    pub pipeline_id: String,
    pub pipeline_name: String,
    pub source: SourceConfig,
    pub tls_terminate: Option<TlsTerminateConfig>,
    pub tls_initiate: Option<TlsInitiateConfig>,
    pub filters: Vec<Filter>,
    pub forward: ForwardConfig,
    pub duplicate: Option<DuplicateConfig>,
    pub exporter: Option<ExporterConfig>,
}

#[derive(Debug)]
pub struct DagPlan {
    pub pipeline_id: String,
    pub pipeline_name: String,
    pub source: SourceConfig,
    pub stages: Vec<DagStage>,
}

#[derive(Debug, Clone)]
pub struct DagStage {
    pub node_id: NodeId,
    pub kind: NodeKind,
    pub outputs: Vec<DagOutput>,
}

#[derive(Debug, Clone)]
pub struct DagOutput {
    pub target_stage_index: usize,
    pub port: Option<String>,
}

#[derive(Debug)]
pub enum CompileError {
    Validation(Vec<validate::ValidationError>),
    Internal(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(errors) => {
                write!(f, "validation errors: ")?;
                for (i, e) in errors.iter().enumerate() {
                    if i > 0 { write!(f, "; ")?; }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
            Self::Internal(msg) => write!(f, "internal: {msg}"),
        }
    }
}

pub fn compile(pipeline: Pipeline) -> Result<ExecutionPlan, CompileError> {
    let result = validate::validate(&pipeline);
    if !result.errors.is_empty() {
        return Err(CompileError::Validation(result.errors));
    }

    let topo = &result.topological_order;

    // Classify: is this linear (every node has ≤1 in and ≤1 out, no Router)?
    let has_router = pipeline.nodes.iter().any(|n| matches!(n.kind, NodeKind::Router(_)));
    let max_outgoing = pipeline.nodes.iter().map(|n| pipeline.outgoing_edges(n.id).len()).max().unwrap_or(0);
    let is_linear = !has_router && max_outgoing <= 1;

    if is_linear {
        compile_linear(pipeline, topo)
    } else {
        compile_dag(pipeline, topo)
    }
}

fn compile_linear(
    pipeline: Pipeline,
    topo: &[NodeId],
) -> Result<ExecutionPlan, CompileError> {
    let mut source: Option<SourceConfig> = None;
    let mut forward: Option<ForwardConfig> = None;
    let mut tls_terminate: Option<TlsTerminateConfig> = None;
    let mut tls_initiate: Option<TlsInitiateConfig> = None;
    let mut filters: Vec<Filter> = Vec::new();
    let mut duplicate: Option<DuplicateConfig> = None;
    let mut exporter: Option<ExporterConfig> = None;
    let mut has_processing = false;

    for &node_id in topo {
        let node = pipeline.node(node_id)
            .ok_or_else(|| CompileError::Internal(format!("missing node {node_id}")))?;

        match &node.kind {
            NodeKind::Source(cfg) => {
                source = Some(cfg.clone());
            }
            NodeKind::Forward(cfg) => {
                forward = Some(cfg.clone());
            }
            NodeKind::TlsTerminate(cfg) => {
                tls_terminate = Some(cfg.clone());
                has_processing = true;
            }
            NodeKind::TlsInitiate(cfg) => {
                tls_initiate = Some(cfg.clone());
                has_processing = true;
            }
            NodeKind::Filter(cfg) => {
                filters.extend(cfg.filters.clone());
                has_processing = true;
            }
            NodeKind::Duplicate(cfg) => {
                duplicate = Some(cfg.clone());
                has_processing = true;
            }
            NodeKind::Export(cfg) => {
                exporter = Some(cfg.exporter.clone());
                has_processing = true;
            }
            NodeKind::Drop => {
                // Drop as a linear sink — pipeline drops all traffic
                has_processing = true;
            }
            NodeKind::Router(_) => {
                return Err(CompileError::Internal("router in linear pipeline".into()));
            }
            #[cfg(feature = "http2")]
            NodeKind::Http2Decode(_) => {
                has_processing = true;
            }
        }
    }

    let source = source.ok_or_else(|| CompileError::Internal("no source".into()))?;

    // Check for Tier 0: simple forward with no processing
    if !has_processing && filters.is_empty() {
        if let Some(fwd) = forward {
            return Ok(ExecutionPlan::FastForward(FastForwardPlan {
                pipeline_id: pipeline.id,
                pipeline_name: pipeline.name,
                source,
                forward: fwd,
            }));
        }
    }

    // Tier 1: Linear
    let forward = forward.ok_or_else(|| CompileError::Internal("no forward node in linear pipeline".into()))?;
    Ok(ExecutionPlan::Linear(LinearPlan {
        pipeline_id: pipeline.id,
        pipeline_name: pipeline.name,
        source,
        tls_terminate,
        tls_initiate,
        filters,
        forward,
        duplicate,
        exporter,
    }))
}

fn compile_dag(
    pipeline: Pipeline,
    topo: &[NodeId],
) -> Result<ExecutionPlan, CompileError> {
    let source_id = pipeline.source_node()
        .ok_or_else(|| CompileError::Internal("no source".into()))?.id;
    let source_cfg = match &pipeline.source_node().unwrap().kind {
        NodeKind::Source(cfg) => cfg.clone(),
        _ => return Err(CompileError::Internal("source node is not Source kind".into())),
    };

    // Build node_id → stage_index mapping (excluding source)
    let stage_nodes: Vec<NodeId> = topo.iter().copied().filter(|&id| id != source_id).collect();
    let mut id_to_index: std::collections::HashMap<NodeId, usize> = std::collections::HashMap::new();
    for (i, &id) in stage_nodes.iter().enumerate() {
        id_to_index.insert(id, i);
    }

    let mut stages: Vec<DagStage> = Vec::new();
    for &node_id in &stage_nodes {
        let node = pipeline.node(node_id)
            .ok_or_else(|| CompileError::Internal(format!("missing node {node_id}")))?;

        let outputs: Vec<DagOutput> = pipeline
            .outgoing_edges(node_id)
            .iter()
            .filter_map(|e| {
                id_to_index.get(&e.to_node).map(|&idx| DagOutput {
                    target_stage_index: idx,
                    port: e.from_port.clone(),
                })
            })
            .collect();

        stages.push(DagStage {
            node_id,
            kind: node.kind.clone(),
            outputs,
        });
    }

    Ok(ExecutionPlan::Dag(DagPlan {
        pipeline_id: pipeline.id,
        pipeline_name: pipeline.name,
        source: source_cfg,
        stages,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::endpoint::Endpoint;
    use crate::types::filter::{Direction, FilterAction, FilterKind};
    use crate::types::rule::UdpSourceMode;

    fn src(id: NodeId) -> Node {
        Node {
            id,
            label: "src".into(),
            kind: NodeKind::Source(SourceConfig {
                endpoint: Endpoint::tcp("127.0.0.1:8080".parse().unwrap()),
                udp_source_mode: UdpSourceMode::Proxy,
                idle_timeout_secs: 30,
            }),
        }
    }

    fn fwd(id: NodeId) -> Node {
        Node {
            id,
            label: "fwd".into(),
            kind: NodeKind::Forward(ForwardConfig {
                endpoint: Endpoint::tcp("127.0.0.1:9090".parse().unwrap()),
                tcp_nodelay: true,
            }),
        }
    }

    fn filter_node(id: NodeId) -> Node {
        Node {
            id,
            label: "filter".into(),
            kind: NodeKind::Filter(FilterNodeConfig {
                filters: vec![Filter {
                    kind: FilterKind::Substr("test".into()),
                    direction: Direction::Both,
                    action_on_match: FilterAction::Drop,
                }],
            }),
        }
    }

    fn edge(id: EdgeId, from: NodeId, to: NodeId) -> Edge {
        Edge { id, from_node: from, to_node: to, from_port: None }
    }

    fn edge_port(id: EdgeId, from: NodeId, to: NodeId, port: &str) -> Edge {
        Edge { id, from_node: from, to_node: to, from_port: Some(port.into()) }
    }

    #[test]
    fn test_compile_fast_forward() {
        let p = Pipeline {
            id: "ff".into(), name: "ff".into(),
            nodes: vec![src(1), fwd(2)],
            edges: vec![edge(1, 1, 2)],
        };
        let plan = compile(p).unwrap();
        assert!(matches!(plan, ExecutionPlan::FastForward(_)));
        assert_eq!(plan.tier(), "fast_forward");
    }

    #[test]
    fn test_compile_linear_with_filter() {
        let p = Pipeline {
            id: "lin".into(), name: "lin".into(),
            nodes: vec![src(1), filter_node(2), fwd(3)],
            edges: vec![edge(1, 1, 2), edge(2, 2, 3)],
        };
        let plan = compile(p).unwrap();
        assert!(matches!(plan, ExecutionPlan::Linear(_)));
        if let ExecutionPlan::Linear(lp) = &plan {
            assert_eq!(lp.filters.len(), 1);
        }
    }

    #[test]
    fn test_compile_dag_with_router() {
        let router = Node {
            id: 2,
            label: "router".into(),
            kind: NodeKind::Router(RouterConfig {
                filter: Filter {
                    kind: FilterKind::Substr("block".into()),
                    direction: Direction::Both,
                    action_on_match: FilterAction::Drop,
                },
            }),
        };
        let p = Pipeline {
            id: "dag".into(), name: "dag".into(),
            nodes: vec![src(1), router, fwd(3), Node { id: 4, label: "drop".into(), kind: NodeKind::Drop }],
            edges: vec![
                edge(1, 1, 2),
                edge_port(2, 2, 3, "default"),
                edge_port(3, 2, 4, "match"),
            ],
        };
        let plan = compile(p).unwrap();
        assert!(matches!(plan, ExecutionPlan::Dag(_)));
        if let ExecutionPlan::Dag(dp) = &plan {
            assert_eq!(dp.stages.len(), 3); // router + forward + drop
        }
    }

    #[test]
    fn test_compile_invalid_pipeline() {
        let p = Pipeline {
            id: "bad".into(), name: "bad".into(),
            nodes: vec![fwd(1)], // no source
            edges: vec![],
        };
        let result = compile(p);
        assert!(result.is_err());
    }
}
