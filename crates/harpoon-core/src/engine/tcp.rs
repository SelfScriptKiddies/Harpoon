use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::engine::executor::TcpParams;
use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::error::HarpoonError;
use crate::types::event::{Event, EventKind};
use crate::types::filter::{Direction, FilterAction};
use crate::types::rule::Rule;
use crate::types::stats::RuleStats;

#[cfg(feature = "tls")]
use crate::tls::cert::CertAuthority;

/// Pipeline-based TCP executor. Accepts parameterized config.
pub async fn run_tcp_pipeline(
    params: TcpParams,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
) -> Result<(), HarpoonError> {
    let listener = TcpListener::bind(params.listen_addr)
        .await
        .map_err(|e| HarpoonError::Bind {
            addr: params.listen_addr,
            source: e,
        })?;

    tracing::info!(pipeline = %params.name, listen = %params.listen_addr, target = %params.target_addr, "tcp pipeline started");
    emit_event(
        &event_tx, &export_tx, &stats,
        EventKind::RuleActivated { rule: params.name.clone() },
    ).await;

    let target_addr = params.target_addr;
    let name = Arc::new(params.name);
    let dup_endpoint = params.duplicate_addr;
    let filters = params.filters;
    let capture = params.capture;
    let buffer_size = params.buffer_size;
    let tcp_nodelay = params.tcp_nodelay;

    #[cfg(feature = "tls")]
    let ca = params.ca;
    #[cfg(feature = "tls")]
    let tls_terminate = params.tls_terminate;
    #[cfg(feature = "tls")]
    let tls_initiate = params.tls_initiate;

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

                let _ = client_stream.set_nodelay(tcp_nodelay);

                stats.active_tcp_connections.fetch_add(1, Ordering::Relaxed);
                emit_event(&event_tx, &export_tx, &stats, EventKind::TcpConnectionOpened {
                    rule: name.to_string(), client: client_addr,
                }).await;

                let stats = stats.clone();
                let filters = filters.clone();
                let event_tx = event_tx.clone();
                let export_tx = export_tx.clone();
                let cancel = cancel.child_token();
                let name = name.clone();
                let capture = capture.clone();

                #[cfg(feature = "tls")]
                let ca = ca.clone();

                tokio::spawn(async move {
                    let result = {
                        #[cfg(feature = "tls")]
                        {
                            if tls_terminate {
                                if let Some(ref ca) = ca {
                                    let mode = if tls_initiate {
                                        crate::types::rule::TlsMode::Mitm
                                    } else {
                                        crate::types::rule::TlsMode::Terminate
                                    };
                                    crate::tls::mitm::handle_tls_connection(
                                        client_stream, client_addr, target_addr,
                                        &mode, ca, &filters, &stats,
                                        &event_tx, &export_tx, &cancel,
                                        buffer_size, &name,
                                    ).await
                                } else {
                                    Err(HarpoonError::Config("TLS mode requires CA certificate".into()))
                                }
                            } else {
                                handle_tcp_connection(
                                    client_stream, client_addr, target_addr,
                                    dup_endpoint, &filters, &stats,
                                    &event_tx, &export_tx, &cancel,
                                    buffer_size, tcp_nodelay, &name, &capture,
                                ).await
                            }
                        }
                        #[cfg(not(feature = "tls"))]
                        {
                            handle_tcp_connection(
                                client_stream, client_addr, target_addr,
                                dup_endpoint, &filters, &stats,
                                &event_tx, &export_tx, &cancel,
                                buffer_size, tcp_nodelay, &name, &capture,
                            ).await
                        }
                    };

                    if let Err(e) = result {
                        tracing::debug!(pipeline = %name, error = %e, "tcp connection ended");
                    }

                    stats.active_tcp_connections.fetch_sub(1, Ordering::Relaxed);
                    let _ = event_tx.send(Event::new(EventKind::TcpConnectionClosed {
                        rule: name.to_string(), client: client_addr,
                    }));
                });
            }
            _ = cancel.cancelled() => {
                tracing::info!(pipeline = %name, "tcp pipeline shutting down");
                break;
            }
        }
    }

    Ok(())
}

/// Backward-compat wrapper: runs a Rule as a TCP pipeline.
pub async fn run_tcp_rule(
    rule: Rule,
    stats: Arc<RuleStats>,
    filters: Arc<Vec<CompiledFilter>>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
    buffer_size: usize,
    tcp_nodelay: bool,
    #[cfg(feature = "tls")] ca: Option<Arc<CertAuthority>>,
) -> Result<(), HarpoonError> {
    let params = TcpParams {
        name: rule.name.clone(),
        listen_addr: rule.listen.addr,
        target_addr: rule.target.addr,
        filters,
        duplicate_addr: rule.duplicate.as_ref().map(|d| d.endpoint.addr),
        buffer_size,
        tcp_nodelay,
        capture: crate::capture::CaptureManager::new(),
        #[cfg(feature = "tls")]
        ca,
        #[cfg(feature = "tls")]
        tls_terminate: rule.tls.as_ref().map(|t| !matches!(t.mode, crate::types::rule::TlsMode::Passthrough)).unwrap_or(false),
        #[cfg(feature = "tls")]
        tls_initiate: rule.tls.as_ref().map(|t| matches!(t.mode, crate::types::rule::TlsMode::Mitm)).unwrap_or(false),
    };
    run_tcp_pipeline(params, stats, event_tx, export_tx, cancel).await
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
    tcp_nodelay: bool,
    rule_name: &str,
    capture: &Arc<crate::capture::CaptureManager>,
) -> Result<(), HarpoonError> {
    let upstream = TcpStream::connect(target_addr)
        .await
        .map_err(|e| HarpoonError::UpstreamConnect {
            addr: target_addr,
            source: e,
        })?;

    let _ = upstream.set_nodelay(tcp_nodelay);

    // Fast path: no filters, no duplicate — use zero-copy bidirectional copy
    if filters.is_empty() && dup_endpoint.is_none() {
        return fast_path_proxy(client_stream, upstream, stats, cancel).await;
    }

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
                            let kind = if action == FilterAction::Drop || action == FilterAction::DropConnection {
                                EventKind::FilterDrop { rule: rule_name.clone(), filter_index: idx }
                            } else {
                                EventKind::FilterMatch { rule: rule_name.clone(), filter_index: idx }
                            };
                            emit_event(&event_tx, &export_tx, stats, kind).await;
                        }

                        let filter_name = filter_idx.map(|i| format!("filter#{i}"));

                        match action {
                            FilterAction::Drop => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                capture.record_with_filter(&rule_name, crate::capture::PacketDirection::ClientToServer, client_addr, target_addr, data, filter_name, true).await;
                                continue;
                            }
                            FilterAction::DropConnection => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                capture.record_with_filter(&rule_name, crate::capture::PacketDirection::ClientToServer, client_addr, target_addr, data, filter_name, true).await;
                                break;
                            }
                            FilterAction::TapOnly => {
                                emit_event(&event_tx, &export_tx, stats, EventKind::IncomingData {
                                    rule: rule_name.clone(), src: client_addr, len: n,
                                }).await;
                                continue;
                            }
                            FilterAction::Pass => {}
                        }

                        upstream_write.write_all(data).await?;
                        stats.bytes_client_to_server.fetch_add(n as u64, Ordering::Relaxed);
                        stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);

                        capture.record_with_filter(&rule_name, crate::capture::PacketDirection::ClientToServer, client_addr, target_addr, data, filter_name, false).await;

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
                            let kind = if action == FilterAction::Drop || action == FilterAction::DropConnection {
                                EventKind::FilterDrop { rule: rule_name.clone(), filter_index: idx }
                            } else {
                                EventKind::FilterMatch { rule: rule_name.clone(), filter_index: idx }
                            };
                            emit_event(&event_tx, &export_tx, stats, kind).await;
                        }

                        let filter_name = filter_idx.map(|i| format!("filter#{i}"));

                        match action {
                            FilterAction::Drop => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                capture.record_with_filter(&rule_name, crate::capture::PacketDirection::ServerToClient, target_addr, client_addr, data, filter_name, true).await;
                                continue;
                            }
                            FilterAction::DropConnection => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                capture.record_with_filter(&rule_name, crate::capture::PacketDirection::ServerToClient, target_addr, client_addr, data, filter_name, true).await;
                                break;
                            }
                            FilterAction::TapOnly => {
                                emit_event(&event_tx, &export_tx, stats, EventKind::OutgoingData {
                                    rule: rule_name.clone(), dst: client_addr, len: n,
                                }).await;
                                continue;
                            }
                            FilterAction::Pass => {}
                        }

                        client_write.write_all(data).await?;
                        stats.bytes_server_to_client.fetch_add(n as u64, Ordering::Relaxed);
                        stats.packets_server_to_client.fetch_add(1, Ordering::Relaxed);

                        capture.record_with_filter(&rule_name, crate::capture::PacketDirection::ServerToClient, target_addr, client_addr, data, filter_name, false).await;
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

/// Fast path for filterless, no-duplicate TCP proxy using copy_bidirectional
async fn fast_path_proxy(
    mut client: TcpStream,
    mut upstream: TcpStream,
    stats: &RuleStats,
    cancel: &CancellationToken,
) -> Result<(), HarpoonError> {
    tokio::select! {
        result = tokio::io::copy_bidirectional(&mut client, &mut upstream) => {
            let (c2s, s2c) = result.map_err(HarpoonError::Io)?;
            stats.bytes_client_to_server.fetch_add(c2s, Ordering::Relaxed);
            stats.bytes_server_to_client.fetch_add(s2c, Ordering::Relaxed);
            Ok(())
        }
        _ = cancel.cancelled() => Ok(()),
    }
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
