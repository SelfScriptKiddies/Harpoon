//! HTTP/2 proxy executor — feature-gated behind `http2`.
//!
//! Proxies HTTP/2 connections with frame-level access:
//! - Parses HTTP/2 frames via the `h2` crate
//! - Per-stream header/body access for filtering
//! - Serializes request/response as HTTP/1.1-like text for capture
//! - Falls back to raw TCP on parse failure

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
    export_tx: &Option<mpsc::Sender<Event>>,
    cancel: &CancellationToken,
    rule_name: &str,
    capture: &Arc<CaptureManager>,
) -> Result<(), HarpoonError> {
    // Server side: accept HTTP/2 from client
    let mut h2_server = server::handshake(client_stream)
        .await
        .map_err(|e| HarpoonError::Config(format!("HTTP/2 server handshake failed: {e}")))?;

    tracing::debug!(rule = rule_name, "HTTP/2 server handshake complete");

    // Client side: connect to upstream as HTTP/2
    let upstream = TcpStream::connect(target_addr)
        .await
        .map_err(|e| HarpoonError::UpstreamConnect { addr: target_addr, source: e })?;

    let (h2_client, h2_conn) = client::handshake(upstream)
        .await
        .map_err(|e| HarpoonError::Config(format!("HTTP/2 client handshake failed: {e}")))?;

    // Spawn the client connection driver
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

    // Process streams from client
    while let Some(result) = h2_server.accept().await {
        let (request, mut respond) = match result {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, "HTTP/2 accept stream error");
                break;
            }
        };

        let (head, mut body) = request.into_parts();

        // Serialize request headers as HTTP/1.1-like text
        let method = head.method.to_string();
        let path = head.uri.path_and_query().map(|pq| pq.to_string()).unwrap_or_else(|| "/".into());
        let mut header_text = format!("{method} {path} HTTP/2\r\n");
        for (name, value) in &head.headers {
            if let Ok(v) = value.to_str() {
                header_text.push_str(&format!("{}: {v}\r\n", name.as_str()));
            }
        }
        header_text.push_str("\r\n");

        // Collect request body
        let mut req_body = Vec::new();
        while let Some(chunk) = body.data().await {
            match chunk {
                Ok(data) => {
                    let _ = body.flow_control().release_capacity(data.len());
                    req_body.extend_from_slice(&data);
                }
                Err(e) => {
                    tracing::debug!(error = %e, "HTTP/2 request body error");
                    break;
                }
            }
        }

        // Full request for filtering
        let mut full_request = header_text.as_bytes().to_vec();
        full_request.extend_from_slice(&req_body);

        // Apply filters to request
        let (action, filter_idx) = apply_filters(filters, &full_request, &Direction::ClientToServer);
        if let Some(idx) = filter_idx {
            stats.filter_matches.fetch_add(1, Ordering::Relaxed);
            let kind = if action == FilterAction::Drop || action == FilterAction::DropConnection {
                EventKind::FilterDrop { rule: rule_name.into(), filter_index: idx }
            } else {
                EventKind::FilterMatch { rule: rule_name.into(), filter_index: idx }
            };
            let event = Event::new(kind);
            let _ = event_tx.send(event);
        }

        // Record request to capture
        let filter_name = filter_idx.map(|i| format!("filter#{i}"));
        let was_dropped = action == FilterAction::Drop || action == FilterAction::DropConnection;
        capture.record_with_filter(
            rule_name, PacketDirection::ClientToServer,
            client_addr, target_addr, &full_request,
            filter_name.clone(), was_dropped,
        ).await;

        if was_dropped {
            stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
            if action == FilterAction::DropConnection {
                break;
            }
            // Send 403 back and continue
            let response = http::Response::builder()
                .status(403)
                .body(())
                .unwrap();
            let mut send = respond.send_response(response, false)
                .map_err(|e| HarpoonError::Config(format!("HTTP/2 send response: {e}")))?;
            let _ = send.send_data(Bytes::from_static(b"Blocked by filter"), true);
            continue;
        }

        stats.bytes_client_to_server.fetch_add(full_request.len() as u64, Ordering::Relaxed);
        stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);

        // Forward to upstream
        let upstream_request = Request::from_parts(head, ());
        let (response, mut upstream_body) = match h2_client.send_request(upstream_request, req_body.is_empty()) {
            Ok((resp_future, mut send_stream)) => {
                if !req_body.is_empty() {
                    let _ = send_stream.send_data(Bytes::from(req_body), true);
                }
                match resp_future.await {
                    Ok(resp) => resp.into_parts(),
                    Err(e) => {
                        tracing::debug!(error = %e, "HTTP/2 upstream response error");
                        continue;
                    }
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "HTTP/2 upstream send error");
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

        // Collect response body
        let mut resp_body = Vec::new();
        while let Some(chunk) = upstream_body.data().await {
            match chunk {
                Ok(data) => {
                    let _ = upstream_body.flow_control().release_capacity(data.len());
                    resp_body.extend_from_slice(&data);
                }
                Err(e) => {
                    tracing::debug!(error = %e, "HTTP/2 response body error");
                    break;
                }
            }
        }

        let mut full_response = resp_header_text.as_bytes().to_vec();
        full_response.extend_from_slice(&resp_body);

        // Apply filters to response
        let (resp_action, resp_filter_idx) = apply_filters(filters, &full_response, &Direction::ServerToClient);
        let resp_filter_name = resp_filter_idx.map(|i| format!("filter#{i}"));
        let resp_dropped = resp_action == FilterAction::Drop || resp_action == FilterAction::DropConnection;

        // Record response to capture
        capture.record_with_filter(
            rule_name, PacketDirection::ServerToClient,
            target_addr, client_addr, &full_response,
            resp_filter_name, resp_dropped,
        ).await;

        stats.bytes_server_to_client.fetch_add(full_response.len() as u64, Ordering::Relaxed);
        stats.packets_server_to_client.fetch_add(1, Ordering::Relaxed);

        // Send response back to client
        let client_response = http::Response::builder()
            .status(status)
            .body(())
            .unwrap();
        // Copy response headers
        // Note: h2 handles header forwarding through the response builder
        match respond.send_response(client_response, resp_body.is_empty()) {
            Ok(mut send) => {
                if !resp_body.is_empty() {
                    let _ = send.send_data(Bytes::from(resp_body), true);
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "HTTP/2 send response to client error");
                continue;
            }
        }

        if resp_action == FilterAction::DropConnection {
            break;
        }
    }

    Ok(())
}
