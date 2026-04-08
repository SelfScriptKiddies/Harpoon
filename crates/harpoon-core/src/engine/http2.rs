//! HTTP/2 proxy executor — feature-gated behind `http2`.
//!
//! Proxies HTTP/2 connections with per-stream filtering and capture:
//! - Parses HTTP/2 frames via the `h2` crate
//! - Applies filters to request/response headers (not body — binary protobuf isn't filterable)
//! - Streams body frames incrementally (no full-body buffering)
//! - Records headers + body prefix to capture
//! - Forwards trailers (critical for gRPC: grpc-status, grpc-message)

use std::sync::atomic::Ordering;
use std::sync::Arc;

use bytes::Bytes;
use h2::server;
use h2::client;
use http::Request;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::capture::{CaptureManager, PacketDirection};
use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::error::HarpoonError;
use crate::types::event::{Event, EventKind};
use crate::types::filter::{Direction, FilterAction};
use crate::types::stats::RuleStats;

/// HTTP/2 connection preface (24 bytes).
pub const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Max bytes of body to buffer for capture recording.
const CAPTURE_BODY_PREFIX: usize = 8192;

/// Check if the first bytes look like an HTTP/2 preface.
pub async fn peek_is_h2(stream: &TcpStream) -> bool {
    let mut buf = [0u8; 24];
    match stream.peek(&mut buf).await {
        Ok(n) if n >= 24 => &buf[..24] == H2_PREFACE,
        _ => false,
    }
}

/// Proxy an HTTP/2 connection: accept from client, connect to upstream.
/// Handles each stream independently, applies filters, records to capture.
#[allow(clippy::too_many_arguments)]
pub async fn http2_proxy(
    client_stream: TcpStream,
    client_addr: std::net::SocketAddr,
    target_addr: std::net::SocketAddr,
    filters: &[CompiledFilter],
    stats: &RuleStats,
    event_tx: &broadcast::Sender<Event>,
    _export_tx: &Option<mpsc::Sender<Event>>,
    cancel: &CancellationToken,
    rule_name: &str,
    capture: &Arc<CaptureManager>,
) -> Result<(), HarpoonError> {
    // TCP_NODELAY is critical for HTTP/2 — Nagle can buffer small frames
    // (SETTINGS, WINDOW_UPDATE, PING) causing handshake timeouts
    let _ = client_stream.set_nodelay(true);

    // Server handshake: reads client preface + SETTINGS, queues server SETTINGS.
    // IMPORTANT: server SETTINGS is NOT flushed to the socket here — it only
    // gets written when the Connection is polled (via accept()).
    let h2_server = server::handshake(client_stream)
        .await
        .map_err(|e| HarpoonError::Config(format!("HTTP/2 server handshake failed: {e}")))?;

    tracing::debug!(rule = rule_name, "HTTP/2 server handshake complete");

    // Immediately spawn the accept loop so the Connection is polled and server
    // SETTINGS is flushed to the client. Without this, the client would time out
    // waiting for SETTINGS during the upstream connect + handshake below.
    let (stream_tx, mut stream_rx) = mpsc::channel::<(
        http::Request<h2::RecvStream>,
        h2::server::SendResponse<Bytes>,
    )>(64);
    let accept_cancel = cancel.child_token();
    tokio::spawn(async move {
        let mut h2_server = h2_server;
        loop {
            tokio::select! {
                result = h2_server.accept() => {
                    match result {
                        Some(Ok(pair)) => {
                            if stream_tx.send(pair).await.is_err() { break; }
                        }
                        Some(Err(e)) => {
                            tracing::debug!(error = %e, "HTTP/2 accept stream error");
                            break;
                        }
                        None => break,
                    }
                }
                _ = accept_cancel.cancelled() => break,
            }
        }
    });

    // Upstream connect + H2 handshake. Server SETTINGS is being flushed
    // concurrently by the accept task above.
    let upstream = TcpStream::connect(target_addr)
        .await
        .map_err(|e| HarpoonError::UpstreamConnect { addr: target_addr, source: e })?;
    let _ = upstream.set_nodelay(true);

    let (h2_client, h2_conn) = client::handshake(upstream)
        .await
        .map_err(|e| HarpoonError::Config(format!("HTTP/2 client handshake failed: {e}")))?;

    // Spawn the upstream connection driver
    let conn_cancel = cancel.child_token();
    tokio::spawn(async move {
        tokio::select! {
            result = h2_conn => {
                if let Err(e) = result {
                    tracing::debug!(error = %e, "HTTP/2 upstream connection closed");
                }
            }
            _ = conn_cancel.cancelled() => {}
        }
    });

    let mut h2_client = h2_client.ready().await
        .map_err(|e| HarpoonError::Config(format!("HTTP/2 client not ready: {e}")))?;

    // Process streams forwarded from the accept task
    while let Some((request, mut respond)) = stream_rx.recv().await {

        let (head, mut body) = request.into_parts();

        // Serialize request headers as HTTP/1.1-like text for filtering and capture
        let method = head.method.to_string();
        let path = head.uri.path_and_query().map(|pq| pq.to_string()).unwrap_or_else(|| "/".into());
        let mut header_text = format!("{method} {path} HTTP/2\r\n");
        for (name, value) in &head.headers {
            if let Ok(v) = value.to_str() {
                header_text.push_str(&format!("{}: {v}\r\n", name.as_str()));
            }
        }
        header_text.push_str("\r\n");
        let header_bytes = header_text.into_bytes();

        // Apply filters to headers BEFORE reading body — prevents OOM on blocked large payloads
        let (action, filter_idx) = apply_filters(filters, &header_bytes, &Direction::ClientToServer);
        if let Some(idx) = filter_idx {
            stats.filter_matches.fetch_add(1, Ordering::Relaxed);
            let kind = if action == FilterAction::Drop || action == FilterAction::DropConnection {
                EventKind::FilterDrop { rule: rule_name.into(), filter_index: idx }
            } else {
                EventKind::FilterMatch { rule: rule_name.into(), filter_index: idx }
            };
            let _ = event_tx.send(Event::new(kind));
        }

        let filter_name = filter_idx.map(|i| format!("filter#{i}"));

        if action == FilterAction::Drop || action == FilterAction::DropConnection {
            stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
            capture.record_with_filter(
                rule_name, PacketDirection::ClientToServer,
                client_addr, target_addr, &header_bytes,
                filter_name, true,
            );
            if action == FilterAction::DropConnection {
                break;
            }
            // Send 403 back without reading body
            let blocked = http::Response::builder().status(403).body(())
                .map_err(|e| HarpoonError::Config(format!("HTTP/2 build 403: {e}")))?;
            let mut send = respond.send_response(blocked, false)
                .map_err(|e| HarpoonError::Config(format!("HTTP/2 send 403: {e}")))?;
            let _ = send.send_data(Bytes::from_static(b"Blocked by filter"), true);
            continue;
        }

        // Ensure upstream has capacity
        h2_client = h2_client.ready().await
            .map_err(|e| HarpoonError::Config(format!("HTTP/2 upstream not ready: {e}")))?;

        // Forward request headers to upstream — body streamed below
        let upstream_request = Request::from_parts(head, ());
        let (resp_future, mut upstream_send) = match h2_client.send_request(upstream_request, false) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::debug!(error = %e, "HTTP/2 upstream send error");
                continue;
            }
        };

        // Stream request body frames from client to upstream
        let mut req_body_bytes: u64 = 0;
        let mut capture_buf = header_bytes.clone();
        while let Some(chunk) = body.data().await {
            match chunk {
                Ok(data) => {
                    let _ = body.flow_control().release_capacity(data.len());
                    req_body_bytes += data.len() as u64;
                    // Capture body prefix
                    if capture_buf.len() < header_bytes.len() + CAPTURE_BODY_PREFIX {
                        let remaining = header_bytes.len() + CAPTURE_BODY_PREFIX - capture_buf.len();
                        capture_buf.extend_from_slice(&data[..data.len().min(remaining)]);
                    }
                    let _ = upstream_send.send_data(data, false);
                }
                Err(e) => {
                    tracing::debug!(error = %e, "HTTP/2 request body error");
                    break;
                }
            }
        }

        // Forward request trailers or end stream
        let req_trailers = body.trailers().await.unwrap_or(None);
        if let Some(trailers) = req_trailers {
            let _ = upstream_send.send_trailers(trailers);
        } else {
            let _ = upstream_send.send_data(Bytes::new(), true);
        }

        stats.bytes_client_to_server.fetch_add(header_bytes.len() as u64 + req_body_bytes, Ordering::Relaxed);
        stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);

        // Record request to capture (headers + body prefix)
        capture.record_with_filter(
            rule_name, PacketDirection::ClientToServer,
            client_addr, target_addr, &capture_buf,
            filter_name, false,
        );

        // Wait for upstream response
        let (response, mut upstream_body) = match resp_future.await {
            Ok(resp) => resp.into_parts(),
            Err(e) => {
                tracing::debug!(error = %e, "HTTP/2 upstream response error");
                continue;
            }
        };

        // Serialize response headers
        let status = response.status;
        let mut resp_header_text = format!("HTTP/2 {}\r\n", status.as_u16());
        for (name, value) in &response.headers {
            if let Ok(v) = value.to_str() {
                resp_header_text.push_str(&format!("{}: {v}\r\n", name.as_str()));
            }
        }
        resp_header_text.push_str("\r\n");
        let resp_header_bytes = resp_header_text.into_bytes();

        // Apply filters to response headers
        let (resp_action, resp_filter_idx) = apply_filters(filters, &resp_header_bytes, &Direction::ServerToClient);
        let resp_filter_name = resp_filter_idx.map(|i| format!("filter#{i}"));
        let resp_dropped = resp_action == FilterAction::Drop || resp_action == FilterAction::DropConnection;

        // Build client response — forward all headers
        let mut builder = http::Response::builder().status(status);
        for (name, value) in &response.headers {
            builder = builder.header(name, value);
        }
        let client_response = builder.body(())
            .map_err(|e| HarpoonError::Config(format!("HTTP/2 build response: {e}")))?;

        // Send response headers to client — body streamed below
        let mut client_send = match respond.send_response(client_response, false) {
            Ok(send) => send,
            Err(e) => {
                tracing::debug!(error = %e, "HTTP/2 send response headers error");
                continue;
            }
        };

        // Stream response body from upstream to client
        let mut resp_body_bytes: u64 = 0;
        let mut resp_capture_buf = resp_header_bytes.clone();
        while let Some(chunk) = upstream_body.data().await {
            match chunk {
                Ok(data) => {
                    let _ = upstream_body.flow_control().release_capacity(data.len());
                    resp_body_bytes += data.len() as u64;
                    // Capture body prefix
                    if resp_capture_buf.len() < resp_header_bytes.len() + CAPTURE_BODY_PREFIX {
                        let remaining = resp_header_bytes.len() + CAPTURE_BODY_PREFIX - resp_capture_buf.len();
                        resp_capture_buf.extend_from_slice(&data[..data.len().min(remaining)]);
                    }
                    let _ = client_send.send_data(data, false);
                }
                Err(e) => {
                    tracing::debug!(error = %e, "HTTP/2 response body error");
                    break;
                }
            }
        }

        // Forward response trailers (critical for gRPC) or end stream
        let resp_trailers = upstream_body.trailers().await.unwrap_or(None);
        if let Some(trailers) = resp_trailers {
            let _ = client_send.send_trailers(trailers);
        } else {
            let _ = client_send.send_data(Bytes::new(), true);
        }

        stats.bytes_server_to_client.fetch_add(resp_header_bytes.len() as u64 + resp_body_bytes, Ordering::Relaxed);
        stats.packets_server_to_client.fetch_add(1, Ordering::Relaxed);

        // Record response to capture (headers + body prefix)
        capture.record_with_filter(
            rule_name, PacketDirection::ServerToClient,
            target_addr, client_addr, &resp_capture_buf,
            resp_filter_name, resp_dropped,
        );

        if resp_action == FilterAction::DropConnection {
            break;
        }
    }

    Ok(())
}
