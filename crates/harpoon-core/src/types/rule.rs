use std::net::SocketAddr;
use std::path::PathBuf;

use super::endpoint::Endpoint;
use super::filter::Filter;

#[derive(Debug, Clone)]
pub struct DuplicateTarget {
    pub endpoint: Endpoint,
}

#[derive(Debug, Clone)]
pub enum ExporterKind {
    Uds { path: PathBuf },
    TcpFramed { addr: SocketAddr },
}

#[derive(Debug, Clone)]
pub struct ExporterConfig {
    pub kind: ExporterKind,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub listen: Endpoint,
    pub target: Endpoint,
    pub filters: Vec<Filter>,
    pub duplicate: Option<DuplicateTarget>,
    pub exporter: Option<ExporterConfig>,
    pub idle_timeout_secs: u64,
}

impl Rule {
    pub fn default_idle_timeout() -> u64 {
        30
    }
}
