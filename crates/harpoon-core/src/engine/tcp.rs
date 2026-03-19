use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::error::HarpoonError;
use crate::types::event::{Event, EventKind};
use crate::types::filter::{Direction, FilterAction};
use crate::types::rule::Rule;
use crate::types::stats::RuleStats;

pub async fn run_tcp_rule(
    rule: Rule,
    stats: Arc<RuleStats>,
    filters: Arc<Vec<CompiledFilter>>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
    buffer_size: usize,
) -> Result<(), HarpoonError> {
    let listener = TcpListener::bind(rule.listen.addr)
        .await
        .map_err(|e| HarpoonError::Bind {
            addr: rule.listen.addr,
            source: e,
        })?;

    tracing::info!(rule = %rule.name, listen = %rule.listen, target = %rule.target, "tcp rule started");
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
    let dup_endpoint = rule.duplicate.as_ref().map(|d| d.endpoint.addr);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (client_stream, client_addr) = match result {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "tcp accept error");
                        continue;
                    }
                };

                stats.active_tcp_connections.fetch_add(1, Ordering::Relaxed);
                emit_event(&event_tx, &export_tx, EventKind::TcpConnectionOpened {
                    rule: rule_name.to_string(),
                    client: client_addr,
                }).await;

                let stats = stats.clone();
                let filters = filters.clone();
                let event_tx = event_tx.clone();
                let export_tx = export_tx.clone();
                let cancel = cancel.child_token();
                let rule_name = rule_name.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_tcp_connection(
                        client_stream,
                        client_addr,
                        target_addr,
                        dup_endpoint,
                        &filters,
                        &stats,
                        &event_tx,
                        &export_tx,
                        &cancel,
                        buffer_size,
                        &rule_name,
                    ).await {
                        tracing::debug!(rule = %rule_name, error = %e, "tcp connection ended");
                    }

                    stats.active_tcp_connections.fetch_sub(1, Ordering::Relaxed);
                    let _ = event_tx.send(Event::new(EventKind::TcpConnectionClosed {
                        rule: rule_name.to_string(),
                        client: client_addr,
                    }));
                });
            }
            _ = cancel.cancelled() => {
                tracing::info!(rule = %rule_name, "tcp rule shutting down");
                break;
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_tcp_connection(
    client_stream: TcpStream,
    client_addr: std::net::SocketAddr,
    target_addr: std::net::SocketAddr,
    dup_endpoint: Option<std::net::SocketAddr>,
    filters: &[CompiledFilter],
    stats: &RuleStats,
    event_tx: &broadcast::Sender<Event>,
    export_tx: &Option<mpsc::Sender<Event>>,
    cancel: &CancellationToken,
    buffer_size: usize,
    rule_name: &str,
) -> Result<(), HarpoonError> {
    let upstream = TcpStream::connect(target_addr)
        .await
        .map_err(|e| HarpoonError::UpstreamConnect {
            addr: target_addr,
            source: e,
        })?;

    let mut dup_stream = if let Some(dup_addr) = dup_endpoint {
        match TcpStream::connect(dup_addr).await {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(addr = %dup_addr, error = %e, "duplicate target connect failed");
                None
            }
        }
    } else {
        None
    };

    let (mut client_read, mut client_write) = client_stream.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();

    let rule_name_owned = rule_name.to_string();

    let c2s = {
        let rule_name = rule_name_owned.clone();
        let event_tx = event_tx.clone();
        let export_tx = export_tx.clone();
        let cancel = cancel.clone();

        let mut c2s_buf = vec![0u8; buffer_size];
        async move {
            loop {
                tokio::select! {
                    result = client_read.read(&mut c2s_buf) => {
                        let n = result?;
                        if n == 0 { break; }
                        let data = &c2s_buf[..n];

                        let (action, filter_idx) = apply_filters(filters, data, &Direction::ClientToServer);
                        if let Some(idx) = filter_idx {
                            stats.filter_matches.fetch_add(1, Ordering::Relaxed);
                            let kind = if action == FilterAction::Drop {
                                EventKind::FilterDrop { rule: rule_name.clone(), filter_index: idx }
                            } else {
                                EventKind::FilterMatch { rule: rule_name.clone(), filter_index: idx }
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
                                    rule: rule_name.clone(), src: client_addr, len: n,
                                }).await;
                                continue;
                            }
                            FilterAction::Pass => {}
                        }

                        upstream_write.write_all(data).await?;
                        stats.bytes_client_to_server.fetch_add(n as u64, Ordering::Relaxed);
                        stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);

                        if let Some(ref mut dup) = dup_stream {
                            let _ = dup.write_all(data).await;
                        }
                    }
                    _ = cancel.cancelled() => break,
                }
            }
            Ok::<(), std::io::Error>(())
        }
    };

    let s2c = {
        let rule_name = rule_name_owned;
        let event_tx = event_tx.clone();
        let export_tx = export_tx.clone();
        let cancel = cancel.clone();

        let mut s2c_buf = vec![0u8; buffer_size];
        async move {
            loop {
                tokio::select! {
                    result = upstream_read.read(&mut s2c_buf) => {
                        let n = result?;
                        if n == 0 { break; }
                        let data = &s2c_buf[..n];

                        let (action, filter_idx) = apply_filters(filters, data, &Direction::ServerToClient);
                        if let Some(idx) = filter_idx {
                            stats.filter_matches.fetch_add(1, Ordering::Relaxed);
                            let kind = if action == FilterAction::Drop {
                                EventKind::FilterDrop { rule: rule_name.clone(), filter_index: idx }
                            } else {
                                EventKind::FilterMatch { rule: rule_name.clone(), filter_index: idx }
                            };
                            emit_event(&event_tx, &export_tx, kind).await;
                        }

                        match action {
                            FilterAction::Drop => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                continue;
                            }
                            FilterAction::TapOnly => {
                                emit_event(&event_tx, &export_tx, EventKind::OutgoingData {
                                    rule: rule_name.clone(), dst: client_addr, len: n,
                                }).await;
                                continue;
                            }
                            FilterAction::Pass => {}
                        }

                        client_write.write_all(data).await?;
                        stats.bytes_server_to_client.fetch_add(n as u64, Ordering::Relaxed);
                        stats.packets_server_to_client.fetch_add(1, Ordering::Relaxed);
                    }
                    _ = cancel.cancelled() => break,
                }
            }
            Ok::<(), std::io::Error>(())
        }
    };

    let (r1, r2) = tokio::join!(c2s, s2c);
    r1.map_err(HarpoonError::Io)?;
    r2.map_err(HarpoonError::Io)?;

    Ok(())
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
