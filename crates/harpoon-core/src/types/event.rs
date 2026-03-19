use std::net::SocketAddr;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum EventKind {
    IncomingData {
        rule: String,
        src: SocketAddr,
        len: usize,
    },
    OutgoingData {
        rule: String,
        dst: SocketAddr,
        len: usize,
    },
    FilterMatch {
        rule: String,
        filter_index: usize,
    },
    FilterDrop {
        rule: String,
        filter_index: usize,
    },
    UdpSessionCreated {
        rule: String,
        client: SocketAddr,
    },
    UdpSessionTimeout {
        rule: String,
        client: SocketAddr,
    },
    TcpConnectionOpened {
        rule: String,
        client: SocketAddr,
    },
    TcpConnectionClosed {
        rule: String,
        client: SocketAddr,
    },
    ExporterError {
        rule: String,
        detail: String,
    },
    RuleActivated {
        rule: String,
    },
}

#[derive(Debug, Clone)]
pub struct Event {
    pub timestamp: SystemTime,
    pub kind: EventKind,
}

impl Event {
    pub fn new(kind: EventKind) -> Self {
        Self {
            timestamp: SystemTime::now(),
            kind,
        }
    }
}
