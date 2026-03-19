use std::net::SocketAddr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HarpoonError {
    #[error("config error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("bind failed on {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    #[error("upstream connect failed to {addr}: {source}")]
    UpstreamConnect {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    #[error("export error: {0}")]
    Export(String),

    #[error("filter error: {0}")]
    Filter(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("engine shutdown")]
    Shutdown,
}
