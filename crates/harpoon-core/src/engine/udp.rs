use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::engine::executor::UdpParams;
use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::error::HarpoonError;
use crate::types::event::{Event, EventKind};
use crate::types::filter::{Direction, FilterAction};
use crate::types::rule::{Rule, UdpSourceMode};
use crate::types::stats::RuleStats;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionKey {
    client_addr: SocketAddr,
}

struct UdpSession {
    upstream_socket: Arc<UdpSocket>,
    last_activity: Instant,
    cancel: CancellationToken,
}

/// Pipeline-based UDP executor.
pub async fn run_udp_pipeline(
    params: UdpParams,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
) -> Result<(), HarpoonError> {
    run_udp_inner(
        params.name, params.listen_addr, params.target_addr,
        params.filters, params.duplicate_addr,
        params.udp_source_mode, params.idle_timeout_secs,
        params.max_datagram, params.capture, stats, event_tx, export_tx,
        cancel, params.force_cancel,
    ).await
}

/// Backward-compat wrapper.
pub async fn run_udp_rule(
    rule: Rule,
    stats: Arc<RuleStats>,
    filters: Arc<Vec<CompiledFilter>>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
    max_datagram: usize,
) -> Result<(), HarpoonError> {
    let force = cancel.child_token();
    run_udp_inner(
        rule.name.clone(), rule.listen.addr, rule.target.addr,
        filters, rule.duplicate.as_ref().map(|d| d.endpoint.addr),
        rule.udp_source_mode.clone(), rule.idle_timeout_secs,
        max_datagram, crate::capture::CaptureManager::new(),
        stats, event_tx, export_tx, cancel, force,
    ).await
}

#[allow(clippy::too_many_arguments)]
async fn run_udp_inner(
    name: String,
    listen_addr: SocketAddr,
    target_addr: SocketAddr,
    filters: Arc<Vec<CompiledFilter>>,
    dup_endpoint: Option<SocketAddr>,
    source_mode: UdpSourceMode,
    idle_timeout_secs: u64,
    max_datagram: usize,
    _capture: Arc<crate::capture::CaptureManager>,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
    force_cancel: CancellationToken,
) -> Result<(), HarpoonError> {
    let listener = Arc::new(
        UdpSocket::bind(listen_addr)
            .await
            .map_err(|e| HarpoonError::Bind {
                addr: listen_addr,
                source: e,
            })?,
    );

    tracing::info!(pipeline = %name, listen = %listen_addr, target = %target_addr, "udp pipeline started");
    emit_event(
        &event_tx,
        &export_tx,
        &stats,
        EventKind::RuleActivated {
            rule: name.clone(),
        },
    )
    .await;

    let rule_name = Arc::new(name);
    let idle_timeout = std::time::Duration::from_secs(idle_timeout_secs);

    // Pre-create persistent duplicate socket
    let dup_socket = if let Some(dup_addr) = dup_endpoint {
        let bind_addr: SocketAddr = if dup_addr.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        match UdpSocket::bind(bind_addr).await {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                tracing::warn!(error = %e, "failed to create duplicate socket");
                None
            }
        }
    } else {
        None
    };

    let sessions: Arc<DashMap<SessionKey, UdpSession>> = Arc::new(DashMap::new());

    // Spawn cleanup task
    let cleanup_sessions = sessions.clone();
    let cleanup_cancel = cancel.child_token();
    let cleanup_stats = stats.clone();
    let cleanup_rule_name = rule_name.clone();
    let cleanup_event_tx = event_tx.clone();
    let cleanup_export_tx = export_tx.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let now = Instant::now();
                    let mut expired = Vec::new();

                    for entry in cleanup_sessions.iter() {
                        if now.duration_since(entry.value().last_activity) > idle_timeout {
                            expired.push(entry.key().clone());
                        }
                    }

                    for key in expired {
                        if let Some((_, session)) = cleanup_sessions.remove(&key) {
                            session.cancel.cancel();
                            cleanup_stats.active_udp_sessions.fetch_sub(1, Ordering::Relaxed);
                            emit_event(&cleanup_event_tx, &cleanup_export_tx, &cleanup_stats, EventKind::UdpSessionTimeout {
                                rule: cleanup_rule_name.to_string(),
                                client: key.client_addr,
                            }).await;
                            tracing::debug!(rule = %cleanup_rule_name, client = %key.client_addr, "udp session expired");
                        }
                    }
                }
                _ = cleanup_cancel.cancelled() => break,
            }
        }
    });

    let mut buf = vec![0u8; max_datagram];

    loop {
        tokio::select! {
            result = listener.recv_from(&mut buf) => {
                let (n, client_addr) = match result {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "udp recv_from error");
                        continue;
                    }
                };

                let data = &buf[..n];
                let key = SessionKey { client_addr };

                // Apply client->server filters
                let (action, filter_idx) = apply_filters(&filters, data, &Direction::ClientToServer);
                if let Some(idx) = filter_idx {
                    stats.filter_matches.fetch_add(1, Ordering::Relaxed);
                    let kind = if action == FilterAction::Drop {
                        EventKind::FilterDrop { rule: rule_name.to_string(), filter_index: idx }
                    } else {
                        EventKind::FilterMatch { rule: rule_name.to_string(), filter_index: idx }
                    };
                    emit_event(&event_tx, &export_tx, &stats, kind).await;
                }

                match action {
                    FilterAction::Drop | FilterAction::DropConnection => {
                        stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    FilterAction::TapOnly => {
                        emit_event(&event_tx, &export_tx, &stats, EventKind::IncomingData {
                            rule: rule_name.to_string(), src: client_addr, len: n,
                        }).await;
                        continue;
                    }
                    FilterAction::Pass => {}
                }

                if !sessions.contains_key(&key) {
                    // Create new session
                    let upstream_socket = match create_session_socket(
                        &source_mode, client_addr, target_addr
                    ).await {
                        Ok(s) => Arc::new(s),
                        Err(e) => {
                            tracing::warn!(error = %e, client = %client_addr, "failed to create upstream socket");
                            continue;
                        }
                    };

                    let session_cancel = force_cancel.child_token();

                    // Spawn reverse path task
                    let recv_socket = upstream_socket.clone();
                    let send_socket = listener.clone();
                    let recv_cancel = session_cancel.child_token();
                    let recv_stats = stats.clone();
                    let recv_rule_name = rule_name.clone();
                    let recv_filters = filters.clone();
                    let recv_event_tx = event_tx.clone();
                    let recv_export_tx = export_tx.clone();
                    let recv_sessions = sessions.clone();
                    let recv_key = key.clone();

                    tokio::spawn(async move {
                        let mut recv_buf = vec![0u8; max_datagram];
                        loop {
                            tokio::select! {
                                result = recv_socket.recv(&mut recv_buf) => {
                                    let n = match result {
                                        Ok(n) => n,
                                        Err(e) => {
                                            tracing::debug!(error = %e, "upstream recv error");
                                            break;
                                        }
                                    };

                                    let data = &recv_buf[..n];

                                    let (action, filter_idx) = apply_filters(&recv_filters, data, &Direction::ServerToClient);
                                    if let Some(idx) = filter_idx {
                                        recv_stats.filter_matches.fetch_add(1, Ordering::Relaxed);
                                        let kind = if action == FilterAction::Drop {
                                            EventKind::FilterDrop { rule: recv_rule_name.to_string(), filter_index: idx }
                                        } else {
                                            EventKind::FilterMatch { rule: recv_rule_name.to_string(), filter_index: idx }
                                        };
                                        emit_event(&recv_event_tx, &recv_export_tx, &recv_stats, kind).await;
                                    }

                                    match action {
                                        FilterAction::Drop | FilterAction::DropConnection => {
                                            recv_stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                        FilterAction::TapOnly => continue,
                                        FilterAction::Pass => {}
                                    }

                                    if let Err(e) = send_socket.send_to(data, client_addr).await {
                                        tracing::debug!(error = %e, "send_to client failed");
                                        break;
                                    }

                                    recv_stats.bytes_server_to_client.fetch_add(n as u64, Ordering::Relaxed);
                                    recv_stats.packets_server_to_client.fetch_add(1, Ordering::Relaxed);

                                    // Update last_activity
                                    if let Some(mut entry) = recv_sessions.get_mut(&recv_key) {
                                        entry.last_activity = Instant::now();
                                    }
                                }
                                _ = recv_cancel.cancelled() => break,
                            }
                        }
                    });

                    let session = UdpSession {
                        upstream_socket,
                        last_activity: Instant::now(),
                        cancel: session_cancel,
                    };
                    sessions.insert(key.clone(), session);
                    stats.active_udp_sessions.fetch_add(1, Ordering::Relaxed);

                    emit_event(&event_tx, &export_tx, &stats, EventKind::UdpSessionCreated {
                        rule: rule_name.to_string(),
                        client: client_addr,
                    }).await;
                }

                if let Some(mut entry) = sessions.get_mut(&key) {
                    entry.last_activity = Instant::now();

                    if let Err(e) = entry.upstream_socket.send(data).await {
                        tracing::debug!(error = %e, "upstream send failed");
                        continue;
                    }

                    stats.bytes_client_to_server.fetch_add(n as u64, Ordering::Relaxed);
                    stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);

                    // Duplicate via persistent socket
                    if let (Some(dup_addr), Some(ref dup_sock)) = (dup_endpoint, &dup_socket) {
                        let _ = dup_sock.send_to(data, dup_addr).await;
                    }
                }
            }
            _ = cancel.cancelled() => {
                tracing::info!(rule = %rule_name, "udp rule shutting down");
                for entry in sessions.iter() {
                    entry.value().cancel.cancel();
                }
                break;
            }
        }
    }

    Ok(())
}

async fn create_session_socket(
    source_mode: &UdpSourceMode,
    client_addr: SocketAddr,
    target_addr: SocketAddr,
) -> Result<UdpSocket, HarpoonError> {
    match source_mode {
        UdpSourceMode::Proxy => create_upstream_socket(target_addr).await,
        UdpSourceMode::Preserve => {
            #[cfg(feature = "transparent-udp")]
            {
                let std_sock = super::udp_transparent::create_transparent_upstream_socket(
                    client_addr, target_addr,
                )?;
                UdpSocket::from_std(std_sock).map_err(|e| {
                    HarpoonError::TransparentSocket(format!("tokio conversion: {e}"))
                })
            }
            #[cfg(not(feature = "transparent-udp"))]
            {
                let _ = client_addr;
                Err(HarpoonError::Config(
                    "transparent-udp feature not enabled; cannot use preserve source mode".into(),
                ))
            }
        }
    }
}

async fn create_upstream_socket(target_addr: SocketAddr) -> Result<UdpSocket, HarpoonError> {
    let bind_addr: SocketAddr = if target_addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };
    let socket = UdpSocket::bind(bind_addr)
        .await
        .map_err(HarpoonError::Io)?;
    socket.connect(target_addr).await.map_err(HarpoonError::Io)?;
    Ok(socket)
}

async fn emit_event(
    event_tx: &broadcast::Sender<Event>,
    export_tx: &Option<mpsc::Sender<Event>>,
    stats: &RuleStats,
    kind: EventKind,
) {
    let event = Event::new(kind);
    let _ = event_tx.send(event.clone());
    if let Some(tx) = export_tx {
        if tx.try_send(event).is_err() {
            stats.export_drops.fetch_add(1, Ordering::Relaxed);
        }
    }
}
