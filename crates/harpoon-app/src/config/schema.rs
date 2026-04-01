use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub rules: Vec<AppRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipelines: Vec<AppPipeline>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPipeline {
    pub id: String,
    pub name: String,
    pub nodes: Vec<AppPipelineNode>,
    #[serde(default)]
    pub edges: Vec<AppPipelineEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPipelineNode {
    pub id: u32,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPipelineEdge {
    pub from: u32,
    pub to: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_max_datagram: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shutdown_timeout_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_bind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_password: Option<String>,
    /// Allowed listen ports. Format: "8082,10000-15000,40000-65535". Empty = all allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_ports: Option<String>,
    #[serde(default)]
    pub nft: NftConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NftConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tproxy_mark: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<NftRuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftRuleConfig {
    pub protocol: String,
    pub match_dport: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_dst: Option<String>,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_addr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRule {
    pub name: String,
    pub protocol: String,
    pub listen: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<AppFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exporter: Option<AppExporter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<AppTls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_source_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTls {
    pub mode: String,
    pub ca_cert: String,
    pub ca_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppFilter {
    pub kind: String,
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppExporter {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,
}
