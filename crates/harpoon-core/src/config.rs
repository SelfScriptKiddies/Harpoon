use crate::types::rule::Rule;

#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub rules: Vec<Rule>,
    pub buffer_size: usize,
    pub udp_max_datagram: usize,
    pub shutdown_timeout_secs: u64,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            buffer_size: 8192,
            udp_max_datagram: 65507,
            shutdown_timeout_secs: 5,
        }
    }
}
