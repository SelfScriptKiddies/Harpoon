use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub addr: SocketAddr,
    pub protocol: Protocol,
}

impl Endpoint {
    pub fn tcp(addr: SocketAddr) -> Self {
        Self {
            addr,
            protocol: Protocol::Tcp,
        }
    }

    pub fn udp(addr: SocketAddr) -> Self {
        Self {
            addr,
            protocol: Protocol::Udp,
        }
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let proto = match self.protocol {
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
        };
        write!(f, "{}://{}", proto, self.addr)
    }
}
