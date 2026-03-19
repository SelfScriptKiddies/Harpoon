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
    pub idle_timeout_secs: Option<u64>,
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
