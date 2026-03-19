use std::path::PathBuf;

use super::endpoint::Endpoint;
use super::filter::Filter;
use super::rule::{ExporterConfig, UdpSourceMode};

pub type NodeId = u32;
pub type EdgeId = u32;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub label: String,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    /// Traffic entry point. Exactly one per pipeline.
    Source(SourceConfig),

    /// Terminate TLS from client, pass plaintext downstream.
    TlsTerminate(TlsTerminateConfig),

    /// Wrap plaintext in TLS before sending upstream.
    TlsInitiate(TlsInitiateConfig),

    /// Apply filters. Pass or drop based on match.
    Filter(FilterNodeConfig),

    /// Connect to upstream and proxy bidirectionally. Sink node.
    Forward(ForwardConfig),

    /// Send a copy of traffic to another endpoint. Sink node.
    Duplicate(DuplicateConfig),

    /// Send events/data to an exporter. Sink node.
    Export(ExportNodeConfig),

    /// Discard traffic. Sink node.
    Drop,

    /// Conditional fan-out: match → one output, no match → another.
    Router(RouterConfig),
}

impl NodeKind {
    pub fn is_source(&self) -> bool {
        matches!(self, NodeKind::Source(_))
    }

    pub fn is_sink(&self) -> bool {
        matches!(
            self,
            NodeKind::Forward(_) | NodeKind::Duplicate(_) | NodeKind::Export(_) | NodeKind::Drop
        )
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            NodeKind::Source(_) => "source",
            NodeKind::TlsTerminate(_) => "tls_terminate",
            NodeKind::TlsInitiate(_) => "tls_initiate",
            NodeKind::Filter(_) => "filter",
            NodeKind::Forward(_) => "forward",
            NodeKind::Duplicate(_) => "duplicate",
            NodeKind::Export(_) => "export",
            NodeKind::Drop => "drop",
            NodeKind::Router(_) => "router",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub endpoint: Endpoint,
    pub udp_source_mode: UdpSourceMode,
    pub idle_timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub struct TlsTerminateConfig {
    pub ca_cert_path: PathBuf,
    pub ca_key_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TlsInitiateConfig {
    pub verify_certs: bool,
}

#[derive(Debug, Clone)]
pub struct FilterNodeConfig {
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone)]
pub struct ForwardConfig {
    pub endpoint: Endpoint,
    pub tcp_nodelay: bool,
}

#[derive(Debug, Clone)]
pub struct DuplicateConfig {
    pub endpoint: Endpoint,
}

#[derive(Debug, Clone)]
pub struct ExportNodeConfig {
    pub exporter: ExporterConfig,
}

#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub filter: Filter,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub id: EdgeId,
    pub from_node: NodeId,
    pub to_node: NodeId,
    /// Port name for nodes with multiple outputs (Router: "match"/"default").
    pub from_port: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Pipeline {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn source_node(&self) -> Option<&Node> {
        self.nodes.iter().find(|n| n.kind.is_source())
    }

    pub fn outgoing_edges(&self, node_id: NodeId) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.from_node == node_id).collect()
    }

    pub fn incoming_edges(&self, node_id: NodeId) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.to_node == node_id).collect()
    }
}
