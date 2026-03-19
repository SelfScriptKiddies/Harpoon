use std::collections::{HashMap, HashSet, VecDeque};

use crate::types::pipeline::{NodeId, Pipeline};

#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    /// Topological order of node IDs (valid only if no cycle errors).
    pub topological_order: Vec<NodeId>,
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    NoSource,
    MultipleSources(Vec<NodeId>),
    CycleDetected,
    UnreachableNode(NodeId),
    NoPathToSink(NodeId),
    DanglingEdge {
        edge_id: u32,
        missing_node: NodeId,
    },
    SourceHasIncoming(NodeId),
    SinkHasOutgoing(NodeId),
    NoSink,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSource => write!(f, "pipeline has no source node"),
            Self::MultipleSources(ids) => write!(f, "multiple source nodes: {ids:?}"),
            Self::CycleDetected => write!(f, "cycle detected in pipeline graph"),
            Self::UnreachableNode(id) => write!(f, "node {id} is unreachable from source"),
            Self::NoPathToSink(id) => write!(f, "node {id} has no path to a sink"),
            Self::DanglingEdge { edge_id, missing_node } => {
                write!(f, "edge {edge_id} references missing node {missing_node}")
            }
            Self::SourceHasIncoming(id) => write!(f, "source node {id} has incoming edges"),
            Self::SinkHasOutgoing(id) => write!(f, "sink node {id} has outgoing edges"),
            Self::NoSink => write!(f, "pipeline has no sink node (forward/export/drop)"),
        }
    }
}

pub fn validate(pipeline: &Pipeline) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let node_ids: HashSet<NodeId> = pipeline.nodes.iter().map(|n| n.id).collect();

    // Check for dangling edges
    for edge in &pipeline.edges {
        if !node_ids.contains(&edge.from_node) {
            errors.push(ValidationError::DanglingEdge {
                edge_id: edge.id,
                missing_node: edge.from_node,
            });
        }
        if !node_ids.contains(&edge.to_node) {
            errors.push(ValidationError::DanglingEdge {
                edge_id: edge.id,
                missing_node: edge.to_node,
            });
        }
    }

    // Find sources and sinks
    let sources: Vec<NodeId> = pipeline
        .nodes
        .iter()
        .filter(|n| n.kind.is_source())
        .map(|n| n.id)
        .collect();

    let sinks: Vec<NodeId> = pipeline
        .nodes
        .iter()
        .filter(|n| n.kind.is_sink())
        .map(|n| n.id)
        .collect();

    if sources.is_empty() {
        errors.push(ValidationError::NoSource);
    } else if sources.len() > 1 {
        errors.push(ValidationError::MultipleSources(sources.clone()));
    }

    if sinks.is_empty() {
        errors.push(ValidationError::NoSink);
    }

    // Source should not have incoming edges
    for &src_id in &sources {
        if pipeline.incoming_edges(src_id).len() > 0 {
            errors.push(ValidationError::SourceHasIncoming(src_id));
        }
    }

    // Sinks should not have outgoing edges
    for &sink_id in &sinks {
        if pipeline.outgoing_edges(sink_id).len() > 0 {
            errors.push(ValidationError::SinkHasOutgoing(sink_id));
        }
    }

    // Topological sort (Kahn's algorithm) — also detects cycles
    let topo_order = topological_sort(pipeline);
    match topo_order {
        Some(order) => {
            // Check reachability from source
            if let Some(&source_id) = sources.first() {
                let reachable = reachable_from(pipeline, source_id);
                for node in &pipeline.nodes {
                    if !node.kind.is_source() && !reachable.contains(&node.id) {
                        errors.push(ValidationError::UnreachableNode(node.id));
                    }
                }

                // Check that all non-sink nodes have a path to at least one sink
                let sink_set: HashSet<NodeId> = sinks.iter().copied().collect();
                for node in &pipeline.nodes {
                    if !node.kind.is_sink() {
                        let downstream = reachable_from(pipeline, node.id);
                        if downstream.intersection(&sink_set).count() == 0 {
                            errors.push(ValidationError::NoPathToSink(node.id));
                        }
                    }
                }
            }

            // Warnings
            let has_forward = pipeline.nodes.iter().any(|n| matches!(n.kind, crate::types::pipeline::NodeKind::Forward(_)));
            if !has_forward {
                warnings.push("pipeline has no Forward node — traffic will not be proxied upstream".into());
            }

            ValidationResult {
                errors,
                warnings,
                topological_order: order,
            }
        }
        None => {
            errors.push(ValidationError::CycleDetected);
            ValidationResult {
                errors,
                warnings,
                topological_order: vec![],
            }
        }
    }
}

/// Kahn's algorithm. Returns None if cycle detected.
fn topological_sort(pipeline: &Pipeline) -> Option<Vec<NodeId>> {
    let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for node in &pipeline.nodes {
        in_degree.entry(node.id).or_insert(0);
        adj.entry(node.id).or_insert_with(Vec::new);
    }

    for edge in &pipeline.edges {
        *in_degree.entry(edge.to_node).or_insert(0) += 1;
        adj.entry(edge.from_node)
            .or_insert_with(Vec::new)
            .push(edge.to_node);
    }

    let mut queue: VecDeque<NodeId> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut order = Vec::new();

    while let Some(node) = queue.pop_front() {
        order.push(node);
        if let Some(neighbors) = adj.get(&node) {
            for &next in neighbors {
                if let Some(deg) = in_degree.get_mut(&next) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    if order.len() == pipeline.nodes.len() {
        Some(order)
    } else {
        None // cycle
    }
}

/// BFS from a start node, returns all reachable node IDs.
fn reachable_from(pipeline: &Pipeline, start: NodeId) -> HashSet<NodeId> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some(node) = queue.pop_front() {
        for edge in pipeline.outgoing_edges(node) {
            if visited.insert(edge.to_node) {
                queue.push_back(edge.to_node);
            }
        }
    }

    visited
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::endpoint::Endpoint;
    use crate::types::pipeline::*;
    use crate::types::rule::UdpSourceMode;

    fn source_node(id: NodeId) -> Node {
        Node {
            id,
            label: "source".into(),
            kind: NodeKind::Source(SourceConfig {
                endpoint: Endpoint::tcp("127.0.0.1:8080".parse().unwrap()),
                udp_source_mode: UdpSourceMode::Proxy,
                idle_timeout_secs: 30,
            }),
        }
    }

    fn forward_node(id: NodeId) -> Node {
        Node {
            id,
            label: "forward".into(),
            kind: NodeKind::Forward(ForwardConfig {
                endpoint: Endpoint::tcp("127.0.0.1:9090".parse().unwrap()),
                tcp_nodelay: true,
            }),
        }
    }

    fn drop_node(id: NodeId) -> Node {
        Node {
            id,
            label: "drop".into(),
            kind: NodeKind::Drop,
        }
    }

    fn edge(id: EdgeId, from: NodeId, to: NodeId) -> Edge {
        Edge {
            id,
            from_node: from,
            to_node: to,
            from_port: None,
        }
    }

    #[test]
    fn test_valid_simple_pipeline() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![source_node(1), forward_node(2)],
            edges: vec![edge(1, 1, 2)],
        };
        let r = validate(&p);
        assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
        assert_eq!(r.topological_order, vec![1, 2]);
    }

    #[test]
    fn test_no_source() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![forward_node(1)],
            edges: vec![],
        };
        let r = validate(&p);
        assert!(r.errors.iter().any(|e| matches!(e, ValidationError::NoSource)));
    }

    #[test]
    fn test_no_sink() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![source_node(1)],
            edges: vec![],
        };
        let r = validate(&p);
        assert!(r.errors.iter().any(|e| matches!(e, ValidationError::NoSink)));
    }

    #[test]
    fn test_cycle_detection() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![
                source_node(1),
                Node { id: 2, label: "a".into(), kind: NodeKind::Drop },
                Node { id: 3, label: "b".into(), kind: NodeKind::Drop },
            ],
            edges: vec![edge(1, 1, 2), edge(2, 2, 3), edge(3, 3, 2)],
        };
        let r = validate(&p);
        assert!(r.errors.iter().any(|e| matches!(e, ValidationError::CycleDetected)));
    }

    #[test]
    fn test_dangling_edge() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![source_node(1)],
            edges: vec![edge(1, 1, 99)],
        };
        let r = validate(&p);
        assert!(r.errors.iter().any(|e| matches!(e, ValidationError::DanglingEdge { .. })));
    }

    #[test]
    fn test_source_with_incoming() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![source_node(1), forward_node(2)],
            edges: vec![edge(1, 1, 2), edge(2, 2, 1)],
        };
        let r = validate(&p);
        assert!(r.errors.iter().any(|e| matches!(e, ValidationError::SourceHasIncoming(_))));
    }

    #[test]
    fn test_unreachable_node() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![source_node(1), forward_node(2), drop_node(3)],
            edges: vec![edge(1, 1, 2)],
        };
        let r = validate(&p);
        assert!(r.errors.iter().any(|e| matches!(e, ValidationError::UnreachableNode(3))));
    }
}
