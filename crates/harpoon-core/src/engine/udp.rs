use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::error::HarpoonError;
use crate::types::event::{Event, EventKind};
use crate::types::filter::{Direction, FilterAction};
use crate::types::rule::Rule;
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

pub async fn run_udp_rule(
    rule: Rule,
    stats: Arc<RuleStats>,
    filters: Arc<Vec<CompiledFilter>>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
    max_datagram: usize,
) -> Result<(), HarpoonError> {
    let listener = Arc::new(
        UdpSocket::bind(rule.listen.addr)
            .await
            .map_err(|e| HarpoonError::Bind {
                addr: rule.listen.addr,
                source: e,
            })?,
    );

    tracing::info!(rule = %rule.name, listen = %rule.listen, target = %rule.target, "udp rule started");
    emit_event(
        &event_tx,
        &export_tx,
        EventKind::RuleActivated {
            rule: rule.name.clone(),
        },
    )
    .await;

    let target_addr = rule.target.addr;
    let rule_name = Arc::new(rule.name.clone());
    let idle_timeout = std::time::Duration::from_secs(rule.idle_timeout_secs);
    let dup_endpoint = rule.duplicate.as_ref().map(|d| d.endpoint.addr);

    let sessions: Arc<Mutex<HashMap<SessionKey, UdpSession>>> =
        Arc::new(Mutex::new(HashMap::new()));

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
                    let mut map = cleanup_sessions.lock().await;
                    let mut expired = Vec::new();

                    for (key, session) in map.iter() {
                        if now.duration_since(session.last_activity) > idle_timeout {
                            expired.push(key.clone());
                        }
                    }

                    for key in expired {
                        if let Some(session) = map.remove(&key) {
                            session.cancel.cancel();
                            cleanup_stats.active_udp_sessions.fetch_sub(1, Ordering::Relaxed);
                            emit_event(&cleanup_event_tx, &cleanup_export_tx, EventKind::UdpSessionTimeout {
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
                    emit_event(&event_tx, &export_tx, kind).await;
                }

                match action {
                    FilterAction::Drop => {
                        stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    FilterAction::TapOnly => {
                        emit_event(&event_tx, &export_tx, EventKind::IncomingData {
                            rule: rule_name.to_string(), src: client_addr, len: n,
                        }).await;
                        continue;
                    }
                    FilterAction::Pass => {}
                }

                let mut map = sessions.lock().await;

                if !map.contains_key(&key) {
                    // Create new session
                    let upstream_socket = match create_upstream_socket(target_addr).await {
                        Ok(s) => Arc::new(s),
                        Err(e) => {
                            tracing::warn!(error = %e, client = %client_addr, "failed to create upstream socket");
                            continue;
                        }
                    };

                    let session_cancel = cancel.child_token();

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

                                    // Apply server->client filters
                                    let (action, filter_idx) = apply_filters(&recv_filters, data, &Direction::ServerToClient);
                                    if let Some(idx) = filter_idx {
                                        recv_stats.filter_matches.fetch_add(1, Ordering::Relaxed);
                                        let kind = if action == FilterAction::Drop {
                                            EventKind::FilterDrop { rule: recv_rule_name.to_string(), filter_index: idx }
                                        } else {
                                            EventKind::FilterMatch { rule: recv_rule_name.to_string(), filter_index: idx }
                                        };
                                        emit_event(&recv_event_tx, &recv_export_tx, kind).await;
                                    }

                                    match action {
                                        FilterAction::Drop => {
                                            recv_stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                            continue;
                                        }
                                        FilterAction::TapOnly => {
                                            continue;
                                        }
                                        FilterAction::Pass => {}
                                    }

                                    if let Err(e) = send_socket.send_to(data, client_addr).await {
                                        tracing::debug!(error = %e, "send_to client failed");
                                        break;
                                    }

                                    recv_stats.bytes_server_to_client.fetch_add(n as u64, Ordering::Relaxed);
                                    recv_stats.packets_server_to_client.fetch_add(1, Ordering::Relaxed);

                                    // Update last_activity
                                    let mut map = recv_sessions.lock().await;
                                    if let Some(session) = map.get_mut(&recv_key) {
                                        session.last_activity = Instant::now();
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
                    map.insert(key.clone(), session);
                    stats.active_udp_sessions.fetch_add(1, Ordering::Relaxed);

                    emit_event(&event_tx, &export_tx, EventKind::UdpSessionCreated {
                        rule: rule_name.to_string(),
                        client: client_addr,
                    }).await;
                }

                if let Some(session) = map.get_mut(&key) {
                    session.last_activity = Instant::now();

                    if let Err(e) = session.upstream_socket.send(data).await {
                        tracing::debug!(error = %e, "upstream send failed");
                        continue;
                    }

                    stats.bytes_client_to_server.fetch_add(n as u64, Ordering::Relaxed);
                    stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);

                    // Duplicate if configured
                    if let Some(dup_addr) = dup_endpoint {
                        if let Ok(dup_sock) = UdpSocket::bind("0.0.0.0:0").await {
                            let _ = dup_sock.send_to(data, dup_addr).await;
                        }
                    }
                }
            }
            _ = cancel.cancelled() => {
                tracing::info!(rule = %rule_name, "udp rule shutting down");
                // Cancel all sessions
                let map = sessions.lock().await;
                for (_, session) in map.iter() {
                    session.cancel.cancel();
                }
                break;
            }
        }
    }

    Ok(())
}

async fn create_upstream_socket(target_addr: SocketAddr) -> Result<UdpSocket, std::io::Error> {
    let bind_addr: SocketAddr = if target_addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };
    let socket = UdpSocket::bind(bind_addr).await?;
    socket.connect(target_addr).await?;
    Ok(socket)
}

async fn emit_event(
    event_tx: &broadcast::Sender<Event>,
    export_tx: &Option<mpsc::Sender<Event>>,
    kind: EventKind,
) {
    let event = Event::new(kind);
    let _ = event_tx.send(event.clone());
    if let Some(tx) = export_tx {
        let _ = tx.try_send(event);
    }
}
