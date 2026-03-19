use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct RuleStats {
    pub bytes_client_to_server: AtomicU64,
    pub bytes_server_to_client: AtomicU64,
    pub packets_client_to_server: AtomicU64,
    pub packets_server_to_client: AtomicU64,
    pub active_tcp_connections: AtomicU64,
    pub active_udp_sessions: AtomicU64,
    pub dropped_packets: AtomicU64,
    pub filter_matches: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct RuleStatsSnapshot {
    pub rule_name: String,
    pub bytes_client_to_server: u64,
    pub bytes_server_to_client: u64,
    pub packets_client_to_server: u64,
    pub packets_server_to_client: u64,
    pub active_tcp_connections: u64,
    pub active_udp_sessions: u64,
    pub dropped_packets: u64,
    pub filter_matches: u64,
}

impl RuleStats {
    pub fn snapshot(&self, rule_name: String) -> RuleStatsSnapshot {
        RuleStatsSnapshot {
            rule_name,
            bytes_client_to_server: self.bytes_client_to_server.load(Ordering::Relaxed),
            bytes_server_to_client: self.bytes_server_to_client.load(Ordering::Relaxed),
            packets_client_to_server: self.packets_client_to_server.load(Ordering::Relaxed),
            packets_server_to_client: self.packets_server_to_client.load(Ordering::Relaxed),
            active_tcp_connections: self.active_tcp_connections.load(Ordering::Relaxed),
            active_udp_sessions: self.active_udp_sessions.load(Ordering::Relaxed),
            dropped_packets: self.dropped_packets.load(Ordering::Relaxed),
            filter_matches: self.filter_matches.load(Ordering::Relaxed),
        }
    }
}
