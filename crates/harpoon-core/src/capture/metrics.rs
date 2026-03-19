//! Time-series metrics ring buffer for sparkline charts.
//! Stores per-rule snapshots at regular intervals.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::types::stats::{RuleStats, RuleStatsSnapshot};

const MAX_HISTORY: usize = 120; // 2 minutes at 1s interval

#[derive(Debug, Clone)]
pub struct MetricPoint {
    pub timestamp_ms: u64,
    pub bytes_in_rate: u64,
    pub bytes_out_rate: u64,
    pub packets_in_rate: u64,
    pub packets_out_rate: u64,
    pub tcp_connections: u64,
    pub udp_sessions: u64,
    pub drops_rate: u64,
}

pub struct MetricsCollector {
    history: Mutex<HashMap<String, Vec<MetricPoint>>>,
    prev_snapshots: Mutex<HashMap<String, RuleStatsSnapshot>>,
}

impl MetricsCollector {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            history: Mutex::new(HashMap::new()),
            prev_snapshots: Mutex::new(HashMap::new()),
        })
    }

    /// Record a new set of snapshots. Call this every interval (e.g. 1 second).
    pub async fn record(&self, snapshots: &[RuleStatsSnapshot]) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut history = self.history.lock().await;
        let mut prev = self.prev_snapshots.lock().await;

        for snap in snapshots {
            let prev_snap = prev.get(&snap.rule_name);
            let point = MetricPoint {
                timestamp_ms: now,
                bytes_in_rate: snap.bytes_client_to_server.saturating_sub(
                    prev_snap.map(|p| p.bytes_client_to_server).unwrap_or(0),
                ),
                bytes_out_rate: snap.bytes_server_to_client.saturating_sub(
                    prev_snap.map(|p| p.bytes_server_to_client).unwrap_or(0),
                ),
                packets_in_rate: snap.packets_client_to_server.saturating_sub(
                    prev_snap.map(|p| p.packets_client_to_server).unwrap_or(0),
                ),
                packets_out_rate: snap.packets_server_to_client.saturating_sub(
                    prev_snap.map(|p| p.packets_server_to_client).unwrap_or(0),
                ),
                tcp_connections: snap.active_tcp_connections,
                udp_sessions: snap.active_udp_sessions,
                drops_rate: snap.dropped_packets.saturating_sub(
                    prev_snap.map(|p| p.dropped_packets).unwrap_or(0),
                ),
            };

            let entry = history.entry(snap.rule_name.clone()).or_insert_with(Vec::new);
            entry.push(point);
            if entry.len() > MAX_HISTORY {
                entry.drain(0..entry.len() - MAX_HISTORY);
            }

            prev.insert(snap.rule_name.clone(), snap.clone());
        }
    }

    /// Get metrics history for a specific rule.
    pub async fn get_rule_history(&self, rule_name: &str) -> Vec<MetricPoint> {
        let history = self.history.lock().await;
        history.get(rule_name).cloned().unwrap_or_default()
    }

    /// Get aggregated metrics across all rules.
    pub async fn get_global_history(&self) -> Vec<MetricPoint> {
        let history = self.history.lock().await;

        // Merge all rules by timestamp
        let mut merged: HashMap<u64, MetricPoint> = HashMap::new();
        for points in history.values() {
            for p in points {
                let entry = merged.entry(p.timestamp_ms).or_insert(MetricPoint {
                    timestamp_ms: p.timestamp_ms,
                    bytes_in_rate: 0, bytes_out_rate: 0,
                    packets_in_rate: 0, packets_out_rate: 0,
                    tcp_connections: 0, udp_sessions: 0, drops_rate: 0,
                });
                entry.bytes_in_rate += p.bytes_in_rate;
                entry.bytes_out_rate += p.bytes_out_rate;
                entry.packets_in_rate += p.packets_in_rate;
                entry.packets_out_rate += p.packets_out_rate;
                entry.tcp_connections += p.tcp_connections;
                entry.udp_sessions += p.udp_sessions;
                entry.drops_rate += p.drops_rate;
            }
        }

        let mut points: Vec<MetricPoint> = merged.into_values().collect();
        points.sort_by_key(|p| p.timestamp_ms);
        if points.len() > MAX_HISTORY {
            points.drain(0..points.len() - MAX_HISTORY);
        }
        points
    }
}

/// Spawn a background task that records metrics every second.
pub fn spawn_metrics_collector(
    collector: Arc<MetricsCollector>,
    stats: Vec<(String, Arc<RuleStats>)>,
    cancel: tokio_util::sync::CancellationToken,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let snapshots: Vec<RuleStatsSnapshot> = stats
                        .iter()
                        .map(|(name, s)| s.snapshot(name.clone()))
                        .collect();
                    collector.record(&snapshots).await;
                }
                _ = cancel.cancelled() => break,
            }
        }
    });
}
