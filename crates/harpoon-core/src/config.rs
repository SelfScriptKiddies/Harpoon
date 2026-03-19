use crate::types::rule::Rule;

#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub rules: Vec<Rule>,
    pub buffer_size: usize,
    pub udp_max_datagram: usize,
    pub shutdown_timeout_secs: u64,
    pub event_channel_capacity: usize,
    pub export_channel_capacity: usize,
    pub tcp_nodelay: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            buffer_size: 8192,
            udp_max_datagram: 65507,
            shutdown_timeout_secs: 5,
            event_channel_capacity: 4096,
            export_channel_capacity: 512,
            tcp_nodelay: true,
        }
    }
}
