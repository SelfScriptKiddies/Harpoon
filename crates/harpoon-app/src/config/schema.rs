use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub rules: Vec<AppRule>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GlobalConfig {
    pub buffer_size: Option<usize>,
    pub udp_max_datagram: Option<usize>,
    pub shutdown_timeout_secs: Option<u64>,
    #[allow(dead_code)]
    pub web_bind: Option<String>,
    #[serde(default)]
    pub nft: NftConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct NftConfig {
    #[serde(default)]
    pub enabled: bool,
    pub tproxy_mark: Option<u32>,
    #[serde(default)]
    pub rules: Vec<NftRuleConfig>,
}

#[derive(Debug, Deserialize)]
pub struct NftRuleConfig {
    pub protocol: String,
    pub match_dport: u16,
    pub match_dst: Option<String>,
    pub action: String,
    pub to_port: Option<u16>,
    pub to_addr: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AppRule {
    pub name: String,
    pub protocol: String,
    pub listen: String,
    pub target: String,
    #[serde(default)]
    pub filters: Vec<AppFilter>,
    pub duplicate: Option<String>,
    pub exporter: Option<AppExporter>,
    pub tls: Option<AppTls>,
    pub udp_source_mode: Option<String>,
    pub idle_timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AppTls {
    pub mode: String,
    pub ca_cert: String,
    pub ca_key: String,
}

#[derive(Debug, Deserialize)]
pub struct AppFilter {
    pub kind: String,
    pub pattern: String,
    pub direction: Option<String>,
    pub action: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AppExporter {
    pub kind: String,
    pub path: Option<String>,
    pub addr: Option<String>,
}
