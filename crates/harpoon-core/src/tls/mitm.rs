use std::sync::atomic::Ordering;
use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::error::HarpoonError;
use crate::types::event::{Event, EventKind};
use crate::types::filter::{Direction, FilterAction};
use crate::types::rule::TlsMode;
use crate::types::stats::RuleStats;

use super::cert::CertAuthority;

/// Handle a TLS connection based on the configured TLS mode.
/// Returns after the connection is fully proxied or an error occurs.
pub async fn handle_tls_connection(
    client_stream: tokio::net::TcpStream,
    client_addr: std::net::SocketAddr,
    target_addr: std::net::SocketAddr,
    tls_mode: &TlsMode,
    ca: &Arc<CertAuthority>,
    filters: &[CompiledFilter],
    stats: &RuleStats,
    event_tx: &broadcast::Sender<Event>,
    export_tx: &Option<mpsc::Sender<Event>>,
    cancel: &tokio_util::sync::CancellationToken,
    buffer_size: usize,
    rule_name: &str,
) -> Result<(), HarpoonError> {
    match tls_mode {
        TlsMode::Passthrough => {
            // No TLS processing — just forward raw bytes (handled by normal TCP path)
            unreachable!("passthrough should not reach TLS handler");
        }

        TlsMode::Terminate => {
            // Peek at ClientHello to extract SNI
            let sni = peek_sni(&client_stream).await.unwrap_or_default();
            let server_name = if sni.is_empty() {
                target_addr.ip().to_string()
            } else {
                sni
            };

            let server_config = ca.get_or_create_server_config(&server_name)?;
            let acceptor = TlsAcceptor::from(server_config);

            let tls_client = acceptor.accept(client_stream).await.map_err(|e| {
                HarpoonError::Config(format!("TLS accept failed: {e}"))
            })?;

            tracing::debug!(rule = rule_name, sni = %server_name, "TLS terminated");
            emit_event(event_tx, export_tx, EventKind::TcpConnectionOpened {
                rule: rule_name.into(),
                client: client_addr,
            }).await;

            // Connect to upstream in plaintext
            let upstream = tokio::net::TcpStream::connect(target_addr)
                .await
                .map_err(|e| HarpoonError::UpstreamConnect {
                    addr: target_addr,
                    source: e,
                })?;

            proxy_bidirectional(
                tls_client, upstream, filters, stats, event_tx, export_tx,
                cancel, buffer_size, rule_name, client_addr,
            )
            .await
        }

        TlsMode::Mitm => {
            let sni = peek_sni(&client_stream).await.unwrap_or_default();
            let server_name = if sni.is_empty() {
                target_addr.ip().to_string()
            } else {
                sni
            };

            // Accept TLS from client
            let server_config = ca.get_or_create_server_config(&server_name)?;
            let acceptor = TlsAcceptor::from(server_config);

            let tls_client = acceptor.accept(client_stream).await.map_err(|e| {
                HarpoonError::Config(format!("TLS accept failed: {e}"))
            })?;

            tracing::debug!(rule = rule_name, sni = %server_name, "TLS MITM: client side terminated");

            // Connect to upstream with TLS
            let mut root_store = rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

            let client_config = ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();

            let connector = TlsConnector::from(Arc::new(client_config));
            let upstream_tcp = tokio::net::TcpStream::connect(target_addr)
                .await
                .map_err(|e| HarpoonError::UpstreamConnect {
                    addr: target_addr,
                    source: e,
                })?;

            let dns_name = ServerName::try_from(server_name.clone())
                .map_err(|e| HarpoonError::Config(format!("invalid server name '{server_name}': {e}")))?;

            let tls_upstream = connector.connect(dns_name, upstream_tcp).await.map_err(|e| {
                HarpoonError::Config(format!("TLS upstream connect failed: {e}"))
            })?;

            tracing::debug!(rule = rule_name, sni = %server_name, "TLS MITM: upstream TLS established");

            proxy_bidirectional(
                tls_client, tls_upstream, filters, stats, event_tx, export_tx,
                cancel, buffer_size, rule_name, client_addr,
            )
            .await
        }
    }
}

async fn proxy_bidirectional<C, U>(
    client: C,
    upstream: U,
    filters: &[CompiledFilter],
    stats: &RuleStats,
    event_tx: &broadcast::Sender<Event>,
    export_tx: &Option<mpsc::Sender<Event>>,
    cancel: &tokio_util::sync::CancellationToken,
    buffer_size: usize,
    rule_name: &str,
    _client_addr: std::net::SocketAddr,
) -> Result<(), HarpoonError>
where
    C: AsyncRead + AsyncWrite + Unpin,
    U: AsyncRead + AsyncWrite + Unpin,
{
    let (mut client_read, mut client_write) = tokio::io::split(client);
    let (mut upstream_read, mut upstream_write) = tokio::io::split(upstream);

    let rule_name_owned = rule_name.to_string();

    let c2s = {
        let rule_name = rule_name_owned.clone();
        let event_tx = event_tx.clone();
        let export_tx = export_tx.clone();
        let cancel = cancel.clone();

        async move {
            let mut buf = vec![0u8; buffer_size];
            loop {
                tokio::select! {
                    result = client_read.read(&mut buf) => {
                        let n = result?;
                        if n == 0 { break; }
                        let data = &buf[..n];

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
                            FilterAction::DropConnection => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            FilterAction::TapOnly => continue,
                            FilterAction::Pass => {}
                        }

                        upstream_write.write_all(data).await?;
                        stats.bytes_client_to_server.fetch_add(n as u64, Ordering::Relaxed);
                        stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);
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

        async move {
            let mut buf = vec![0u8; buffer_size];
            loop {
                tokio::select! {
                    result = upstream_read.read(&mut buf) => {
                        let n = result?;
                        if n == 0 { break; }
                        let data = &buf[..n];

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
                            FilterAction::DropConnection => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            FilterAction::TapOnly => continue,
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

/// Peek at the TLS ClientHello to extract SNI without consuming the stream.
/// Uses tokio's peek() on the TCP stream.
async fn peek_sni(stream: &tokio::net::TcpStream) -> Option<String> {
    let mut buf = [0u8; 1024];
    let n = stream.peek(&mut buf).await.ok()?;
    if n < 5 {
        return None;
    }

    // TLS record: type=0x16 (handshake), version, length
    if buf[0] != 0x16 {
        return None;
    }

    let record_len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
    let total = 5 + record_len.min(n - 5);
    let handshake = &buf[5..total];

    if handshake.is_empty() || handshake[0] != 0x01 {
        return None; // Not ClientHello
    }

    // Skip handshake header (4 bytes), client version (2), random (32)
    let mut pos = 4 + 2 + 32;
    if pos >= handshake.len() {
        return None;
    }

    // Session ID
    let session_len = handshake[pos] as usize;
    pos += 1 + session_len;
    if pos + 2 > handshake.len() {
        return None;
    }

    // Cipher suites
    let cipher_len = u16::from_be_bytes([handshake[pos], handshake[pos + 1]]) as usize;
    pos += 2 + cipher_len;
    if pos + 1 > handshake.len() {
        return None;
    }

    // Compression methods
    let comp_len = handshake[pos] as usize;
    pos += 1 + comp_len;
    if pos + 2 > handshake.len() {
        return None;
    }

    // Extensions length
    let ext_len = u16::from_be_bytes([handshake[pos], handshake[pos + 1]]) as usize;
    pos += 2;
    let ext_end = (pos + ext_len).min(handshake.len());

    while pos + 4 <= ext_end {
        let ext_type = u16::from_be_bytes([handshake[pos], handshake[pos + 1]]);
        let ext_data_len = u16::from_be_bytes([handshake[pos + 2], handshake[pos + 3]]) as usize;
        pos += 4;

        if ext_type == 0x0000 {
            // SNI extension
            if pos + 2 > ext_end {
                return None;
            }
            let _sni_list_len = u16::from_be_bytes([handshake[pos], handshake[pos + 1]]);
            pos += 2;
            if pos + 3 > ext_end {
                return None;
            }
            let _name_type = handshake[pos];
            let name_len =
                u16::from_be_bytes([handshake[pos + 1], handshake[pos + 2]]) as usize;
            pos += 3;
            if pos + name_len > ext_end {
                return None;
            }
            return std::str::from_utf8(&handshake[pos..pos + name_len])
                .ok()
                .map(|s| s.to_string());
        }

        pos += ext_data_len;
    }

    None
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
