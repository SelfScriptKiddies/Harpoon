use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Ping,
    Stop,
    Status,
    Stats,
    RulesList,
    Events { count: Option<usize> },
    Reload { config_path: Option<String> },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Pong,
    Error { message: String },
    Status(StatusInfo),
    Stats(Vec<RuleStatsInfo>),
    Rules(Vec<RuleInfo>),
    Events(Vec<EventInfo>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusInfo {
    pub running: bool,
    pub uptime_secs: u64,
    pub rules_count: usize,
    pub config_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuleStatsInfo {
    pub rule_name: String,
    pub bytes_client_to_server: u64,
    pub bytes_server_to_client: u64,
    pub packets_client_to_server: u64,
    pub packets_server_to_client: u64,
    pub active_tcp_connections: u64,
    pub active_udp_sessions: u64,
    pub dropped_packets: u64,
    pub filter_matches: u64,
    pub export_drops: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuleInfo {
    pub name: String,
    pub protocol: String,
    pub listen: String,
    pub target: String,
    pub filters_count: usize,
    pub has_duplicate: bool,
    pub has_exporter: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventInfo {
    pub timestamp_ms: u64,
    pub kind: String,
    pub detail: String,
}

impl From<harpoon_core::types::stats::RuleStatsSnapshot> for RuleStatsInfo {
    fn from(s: harpoon_core::types::stats::RuleStatsSnapshot) -> Self {
        Self {
            rule_name: s.rule_name,
            bytes_client_to_server: s.bytes_client_to_server,
            bytes_server_to_client: s.bytes_server_to_client,
            packets_client_to_server: s.packets_client_to_server,
            packets_server_to_client: s.packets_server_to_client,
            active_tcp_connections: s.active_tcp_connections,
            active_udp_sessions: s.active_udp_sessions,
            dropped_packets: s.dropped_packets,
            filter_matches: s.filter_matches,
            export_drops: s.export_drops,
        }
    }
}
