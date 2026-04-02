use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::broadcast;

const BROADCAST_CAPACITY: usize = 16384;
/// Skip live broadcast when channel exceeds this fill level (75%).
const BROADCAST_PRESSURE_THRESHOLD: usize = BROADCAST_CAPACITY * 3 / 4;

/// Direction of captured packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDirection {
    ClientToServer,
    ServerToClient,
}

impl PacketDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClientToServer => "c2s",
            Self::ServerToClient => "s2c",
        }
    }
}

/// A captured packet with metadata and truncated payload.
#[derive(Debug, Clone)]
pub struct CapturedPacket {
    pub timestamp_ms: u64,
    pub rule_name: String,
    pub direction: PacketDirection,
    pub src: SocketAddr,
    pub dst: SocketAddr,
    pub payload_len: usize,
    /// Payload truncated to max_payload_size. Bytes is O(1) to clone.
    pub payload: Bytes,
    /// Name/index of the filter that matched (if any).
    pub filter_matched: Option<String>,
    /// Whether this packet was dropped by a filter.
    pub was_dropped: bool,
}

/// Configuration for an active capture session.
#[derive(Debug, Clone)]
pub struct CaptureSession {
    pub rule_name: String,
    pub max_packets: usize,
    pub max_payload_size: usize,
    pub started_at: Instant,
    pub timeout: Duration,
}

/// Per-rule capture buffer.
struct RuleCapture {
    session: CaptureSession,
    packets: VecDeque<CapturedPacket>,
}

/// The capture manager — shared across all engine tasks.
/// Uses DashMap for per-shard locking instead of a global Mutex.
pub struct CaptureManager {
    captures: DashMap<String, RuleCapture>,
    /// Broadcast for real-time streaming to WebSocket clients.
    live_tx: broadcast::Sender<CapturedPacket>,
    /// Packets skipped from broadcast due to backpressure.
    broadcast_drops: AtomicU64,
}

impl CaptureManager {
    pub fn new() -> Arc<Self> {
        let (live_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Arc::new(Self {
            captures: DashMap::new(),
            live_tx,
            broadcast_drops: AtomicU64::new(0),
        })
    }

    /// Number of packets skipped from live broadcast due to backpressure.
    pub fn broadcast_drops(&self) -> u64 {
        self.broadcast_drops.load(Ordering::Relaxed)
    }

    /// Start capturing for a rule. Returns error if already capturing.
    pub fn start(
        &self,
        rule_name: String,
        max_packets: usize,
        max_payload_size: usize,
        timeout_secs: u64,
    ) -> Result<(), String> {
        // Clean expired sessions
        let now = Instant::now();
        self.captures.retain(|_, rc| now.duration_since(rc.session.started_at) < rc.session.timeout);

        if self.captures.contains_key(&rule_name) {
            return Err(format!("capture already active for rule '{rule_name}'"));
        }

        self.captures.insert(
            rule_name.clone(),
            RuleCapture {
                session: CaptureSession {
                    rule_name,
                    max_packets,
                    max_payload_size,
                    started_at: now,
                    timeout: Duration::from_secs(timeout_secs),
                },
                packets: VecDeque::with_capacity(max_packets.min(10000)),
            },
        );
        Ok(())
    }

    /// Stop capturing for a rule.
    pub fn stop(&self, rule_name: &str) -> Result<Vec<CapturedPacket>, String> {
        match self.captures.remove(rule_name) {
            Some((_, rc)) => Ok(rc.packets.into()),
            None => Err(format!("no active capture for rule '{rule_name}'")),
        }
    }

    /// Check if capture is active for a rule.
    pub fn is_active(&self, rule_name: &str) -> bool {
        match self.captures.get(rule_name) {
            Some(rc) => Instant::now().duration_since(rc.session.started_at) < rc.session.timeout,
            None => false,
        }
    }

    /// Record a packet. Called from engine hot path — only when capture is active.
    pub fn record(
        &self,
        rule_name: &str,
        direction: PacketDirection,
        src: SocketAddr,
        dst: SocketAddr,
        payload: &[u8],
    ) {
        self.record_with_filter(rule_name, direction, src, dst, payload, None, false);
    }

    /// Record a packet with filter metadata.
    pub fn record_with_filter(
        &self,
        rule_name: &str,
        direction: PacketDirection,
        src: SocketAddr,
        dst: SocketAddr,
        payload: &[u8],
        filter_matched: Option<String>,
        was_dropped: bool,
    ) {
        let mut rc = match self.captures.get_mut(rule_name) {
            Some(rc) => rc,
            None => return,
        };

        // Check timeout
        if Instant::now().duration_since(rc.session.started_at) >= rc.session.timeout {
            return;
        }

        let truncated = &payload[..payload.len().min(rc.session.max_payload_size)];

        let packet = CapturedPacket {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            rule_name: rule_name.to_string(),
            direction,
            src,
            dst,
            payload_len: payload.len(),
            payload: Bytes::copy_from_slice(truncated),
            filter_matched,
            was_dropped,
        };

        // Evict oldest if at capacity
        if rc.packets.len() >= rc.session.max_packets {
            rc.packets.pop_front();
        }
        rc.packets.push_back(packet.clone());

        // Drop the DashMap guard before broadcast to minimize lock hold time
        drop(rc);

        // Adaptive broadcast: skip when channel is under pressure
        if self.live_tx.len() < BROADCAST_PRESSURE_THRESHOLD {
            let _ = self.live_tx.send(packet);
        } else {
            self.broadcast_drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get captured packets for a rule.
    pub fn get_packets(
        &self,
        rule_name: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<CapturedPacket> {
        match self.captures.get(rule_name) {
            Some(rc) => rc
                .packets
                .iter()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect(),
            None => vec![],
        }
    }

    /// List active capture sessions.
    pub fn list_sessions(&self) -> Vec<CaptureSessionInfo> {
        let now = Instant::now();
        self.captures
            .iter()
            .filter(|entry| now.duration_since(entry.session.started_at) < entry.session.timeout)
            .map(|entry| CaptureSessionInfo {
                rule_name: entry.session.rule_name.clone(),
                packets_captured: entry.packets.len(),
                max_packets: entry.session.max_packets,
                elapsed_secs: now.duration_since(entry.session.started_at).as_secs(),
                timeout_secs: entry.session.timeout.as_secs(),
            })
            .collect()
    }

    /// Subscribe to live packet stream.
    pub fn subscribe(&self) -> broadcast::Receiver<CapturedPacket> {
        self.live_tx.subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct CaptureSessionInfo {
    pub rule_name: String,
    pub packets_captured: usize,
    pub max_packets: usize,
    pub elapsed_secs: u64,
    pub timeout_secs: u64,
}
