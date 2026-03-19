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
pub enum TlsMode {
    /// No TLS processing — forward raw bytes
    Passthrough,
    /// Terminate TLS from client, connect to upstream in plaintext
    Terminate,
    /// Full MITM: terminate client TLS, re-encrypt to upstream
    Mitm,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub mode: TlsMode,
    pub ca_cert_path: PathBuf,
    pub ca_key_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub listen: Endpoint,
    pub target: Endpoint,
    pub filters: Vec<Filter>,
    pub duplicate: Option<DuplicateTarget>,
    pub exporter: Option<ExporterConfig>,
    pub tls: Option<TlsConfig>,
    pub idle_timeout_secs: u64,
}

impl Rule {
    pub fn default_idle_timeout() -> u64 {
        30
    }
}
