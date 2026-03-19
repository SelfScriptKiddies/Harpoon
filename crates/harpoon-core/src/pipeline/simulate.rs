use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::types::filter::{Direction, FilterAction};
use crate::types::pipeline::*;

/// Result of simulating a payload through a pipeline.
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub steps: Vec<SimulationStep>,
    pub final_action: String,
}

#[derive(Debug, Clone)]
pub struct SimulationStep {
    pub node_id: NodeId,
    pub node_kind: String,
    pub node_label: String,
    pub action: String,
    pub detail: String,
}

/// Simulate a payload through a compiled pipeline (linear traversal).
/// This doesn't actually send network traffic — it walks the DAG and evaluates filters.
pub fn simulate(pipeline: &Pipeline, payload: &[u8], direction: Direction) -> SimulationResult {
    let mut steps = Vec::new();
    let mut current_data = payload;
    let mut dropped = false;

    // Get topological order
    let topo = match crate::pipeline::validate::validate(pipeline)
        .topological_order
        .is_empty()
    {
        true => return SimulationResult {
            steps: vec![SimulationStep {
                node_id: 0,
                node_kind: "error".into(),
                node_label: "Validation".into(),
                action: "error".into(),
                detail: "Pipeline failed validation".into(),
            }],
            final_action: "error".into(),
        },
        false => crate::pipeline::validate::validate(pipeline).topological_order,
    };

    for &node_id in &topo {
        let node = match pipeline.node(node_id) {
            Some(n) => n,
            None => continue,
        };

        match &node.kind {
            NodeKind::Source(cfg) => {
                steps.push(SimulationStep {
                    node_id,
                    node_kind: "source".into(),
                    node_label: node.label.clone(),
                    action: "accept".into(),
                    detail: format!("Ingress on {}", cfg.endpoint),
                });
            }

            NodeKind::Filter(cfg) => {
                let compiled: Vec<CompiledFilter> = cfg
                    .filters
                    .iter()
                    .filter_map(|f| CompiledFilter::new(f.clone()).ok())
                    .collect();

                let (action, matched_idx) = apply_filters(&compiled, current_data, &direction);

                let (action_str, detail) = match action {
                    FilterAction::Pass => ("pass".into(), match matched_idx {
                        Some(i) => format!("Filter #{} matched, action=pass", i),
                        None => "No filters matched, default pass".into(),
                    }),
                    FilterAction::Drop => {
                        dropped = true;
                        ("drop".into(), format!("Filter #{} matched, action=drop", matched_idx.unwrap_or(0)))
                    }
                    FilterAction::TapOnly => ("tap-only".into(), format!("Filter #{} matched, tap-only", matched_idx.unwrap_or(0))),
                };

                steps.push(SimulationStep {
                    node_id,
                    node_kind: "filter".into(),
                    node_label: node.label.clone(),
                    action: action_str,
                    detail,
                });

                if dropped {
                    break;
                }
            }

            NodeKind::TlsTerminate(_) => {
                steps.push(SimulationStep {
                    node_id,
                    node_kind: "tls_terminate".into(),
                    node_label: node.label.clone(),
                    action: "decrypt".into(),
                    detail: "TLS terminated, plaintext available".into(),
                });
            }

            NodeKind::TlsInitiate(_) => {
                steps.push(SimulationStep {
                    node_id,
                    node_kind: "tls_initiate".into(),
                    node_label: node.label.clone(),
                    action: "encrypt".into(),
                    detail: "TLS initiated to upstream".into(),
                });
            }

            NodeKind::Forward(cfg) => {
                steps.push(SimulationStep {
                    node_id,
                    node_kind: "forward".into(),
                    node_label: node.label.clone(),
                    action: "forward".into(),
                    detail: format!("Forward to {}", cfg.endpoint),
                });
            }

            NodeKind::Duplicate(cfg) => {
                steps.push(SimulationStep {
                    node_id,
                    node_kind: "duplicate".into(),
                    node_label: node.label.clone(),
                    action: "copy".into(),
                    detail: format!("Duplicate to {}", cfg.endpoint),
                });
            }

            NodeKind::Export(cfg) => {
                steps.push(SimulationStep {
                    node_id,
                    node_kind: "export".into(),
                    node_label: node.label.clone(),
                    action: "export".into(),
                    detail: format!("Export via {:?}", cfg.exporter.kind),
                });
            }

            NodeKind::Drop => {
                dropped = true;
                steps.push(SimulationStep {
                    node_id,
                    node_kind: "drop".into(),
                    node_label: node.label.clone(),
                    action: "drop".into(),
                    detail: "Traffic dropped".into(),
                });
            }

            NodeKind::Router(cfg) => {
                let compiled: Vec<CompiledFilter> = vec![
                    CompiledFilter::new(cfg.filter.clone()).ok()
                ].into_iter().flatten().collect();

                let (action, _) = apply_filters(&compiled, current_data, &direction);
                let matched = matches!(action, FilterAction::Drop);

                steps.push(SimulationStep {
                    node_id,
                    node_kind: "router".into(),
                    node_label: node.label.clone(),
                    action: if matched { "match".into() } else { "default".into() },
                    detail: format!("Route: {}", if matched { "match branch" } else { "default branch" }),
                });
            }
        }
    }

    let final_action = if dropped {
        "dropped".into()
    } else if steps.iter().any(|s| s.node_kind == "forward") {
        "forwarded".into()
    } else {
        "no_sink".into()
    };

    SimulationResult { steps, final_action }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::endpoint::Endpoint;
    use crate::types::filter::{Filter, FilterKind};
    use crate::types::rule::UdpSourceMode;

    #[test]
    fn test_simulate_simple_forward() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![
                Node { id: 1, label: "src".into(), kind: NodeKind::Source(SourceConfig {
                    endpoint: Endpoint::tcp("127.0.0.1:8080".parse().unwrap()),
                    udp_source_mode: UdpSourceMode::Proxy,
                    idle_timeout_secs: 30,
                }) },
                Node { id: 2, label: "fwd".into(), kind: NodeKind::Forward(ForwardConfig {
                    endpoint: Endpoint::tcp("127.0.0.1:9090".parse().unwrap()),
                    tcp_nodelay: true,
                }) },
            ],
            edges: vec![Edge { id: 1, from_node: 1, to_node: 2, from_port: None }],
        };

        let result = simulate(&p, b"hello world", Direction::ClientToServer);
        assert_eq!(result.final_action, "forwarded");
        assert_eq!(result.steps.len(), 2);
    }

    #[test]
    fn test_simulate_filter_drop() {
        let p = Pipeline {
            id: "test".into(),
            name: "test".into(),
            nodes: vec![
                Node { id: 1, label: "src".into(), kind: NodeKind::Source(SourceConfig {
                    endpoint: Endpoint::tcp("127.0.0.1:8080".parse().unwrap()),
                    udp_source_mode: UdpSourceMode::Proxy,
                    idle_timeout_secs: 30,
                }) },
                Node { id: 2, label: "filter".into(), kind: NodeKind::Filter(FilterNodeConfig {
                    filters: vec![Filter {
                        kind: FilterKind::Substr("blocked".into()),
                        direction: Direction::Both,
                        action_on_match: FilterAction::Drop,
                    }],
                }) },
                Node { id: 3, label: "fwd".into(), kind: NodeKind::Forward(ForwardConfig {
                    endpoint: Endpoint::tcp("127.0.0.1:9090".parse().unwrap()),
                    tcp_nodelay: true,
                }) },
            ],
            edges: vec![
                Edge { id: 1, from_node: 1, to_node: 2, from_port: None },
                Edge { id: 2, from_node: 2, to_node: 3, from_port: None },
            ],
        };

        let result = simulate(&p, b"this is blocked data", Direction::ClientToServer);
        assert_eq!(result.final_action, "dropped");
        assert!(result.steps.iter().any(|s| s.action == "drop"));

        let result2 = simulate(&p, b"allowed data", Direction::ClientToServer);
        assert_eq!(result2.final_action, "forwarded");
    }
}
