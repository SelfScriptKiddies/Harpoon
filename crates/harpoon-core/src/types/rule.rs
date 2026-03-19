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
    /// Pipe to an external process stdin (for Red Eye integration)
    Pipe { command: String, args: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct ExporterConfig {
    pub kind: ExporterKind,
}

#[derive(Debug, Clone)]
pub enum TlsMode {
    Passthrough,
    Terminate,
    Mitm,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub mode: TlsMode,
    pub ca_cert_path: PathBuf,
    pub ca_key_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpSourceMode {
    /// Upstream sees Harpoon's IP (default, works without privileges)
    Proxy,
    /// Upstream sees original client IP (requires CAP_NET_ADMIN + transparent-udp feature)
    Preserve,
}

impl Default for UdpSourceMode {
    fn default() -> Self {
        Self::Proxy
    }
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
    pub udp_source_mode: UdpSourceMode,
    pub idle_timeout_secs: u64,
}

impl Rule {
    pub fn default_idle_timeout() -> u64 {
        30
    }
}
