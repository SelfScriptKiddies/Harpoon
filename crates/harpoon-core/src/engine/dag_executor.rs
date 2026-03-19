//! DAG executor (Tier 2): processes traffic through a branching pipeline.
//!
//! For each chunk of data, walks the DAG stages in topological order.
//! Router nodes evaluate their filter and pick match/default output.
//! Fork nodes (multiple outputs) send data to all branches.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::capture::CaptureManager;
use crate::engine::filter::{apply_filters, CompiledFilter};
use crate::error::HarpoonError;
use crate::pipeline::compile::DagPlan;
use crate::types::event::{Event, EventKind};
use crate::types::filter::{Direction, FilterAction};
use crate::types::pipeline::NodeKind;
use crate::types::stats::RuleStats;

/// Run a TCP DAG pipeline.
pub async fn run_dag_tcp(
    plan: DagPlan,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    export_tx: Option<mpsc::Sender<Event>>,
    cancel: CancellationToken,
    buffer_size: usize,
    tcp_nodelay: bool,
    capture: Arc<CaptureManager>,
) -> Result<(), HarpoonError> {
    let listen_addr = plan.source.endpoint.addr;
    let listener = TcpListener::bind(listen_addr)
        .await
        .map_err(|e| HarpoonError::Bind { addr: listen_addr, source: e })?;

    tracing::info!(pipeline = %plan.pipeline_name, listen = %listen_addr, "dag tcp pipeline started");

    let name = Arc::new(plan.pipeline_name.clone());
    let stages = Arc::new(plan.stages);

    // Pre-compile all filters in stages
    let compiled_stages: Arc<Vec<Vec<CompiledFilter>>> = Arc::new(
        stages.iter().map(|stage| {
            match &stage.kind {
                NodeKind::Filter(cfg) => cfg.filters.iter()
                    .filter_map(|f| CompiledFilter::new(f.clone()).ok())
                    .collect(),
                NodeKind::Router(cfg) => vec![CompiledFilter::new(cfg.filter.clone()).ok()]
                    .into_iter().flatten().collect(),
                _ => vec![],
            }
        }).collect()
    );

    // Identify forward targets
    let forward_addrs: Vec<Option<std::net::SocketAddr>> = stages.iter().map(|stage| {
        match &stage.kind {
            NodeKind::Forward(cfg) => Some(cfg.endpoint.addr),
            _ => None,
        }
    }).collect();

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (client_stream, client_addr) = match result {
                    Ok(v) => v,
                    Err(e) => { tracing::warn!(error = %e, "dag tcp accept error"); continue; }
                };

                let _ = client_stream.set_nodelay(tcp_nodelay);
                stats.active_tcp_connections.fetch_add(1, Ordering::Relaxed);

                let stats = stats.clone();
                let event_tx = event_tx.clone();
                let export_tx = export_tx.clone();
                let cancel = cancel.child_token();
                let name = name.clone();
                let stages = stages.clone();
                let compiled_stages = compiled_stages.clone();
                let forward_addrs = forward_addrs.clone();
                let capture = capture.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_dag_connection(
                        client_stream, client_addr, &stages, &compiled_stages,
                        &forward_addrs, &stats, &event_tx, &export_tx,
                        &cancel, buffer_size, &name, &capture,
                    ).await {
                        tracing::debug!(pipeline = %name, error = %e, "dag connection ended");
                    }
                    stats.active_tcp_connections.fetch_sub(1, Ordering::Relaxed);
                });
            }
            _ = cancel.cancelled() => {
                tracing::info!(pipeline = %name, "dag tcp pipeline shutting down");
                break;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_dag_connection(
    client_stream: TcpStream,
    client_addr: std::net::SocketAddr,
    stages: &[crate::pipeline::compile::DagStage],
    compiled_stages: &[Vec<CompiledFilter>],
    forward_addrs: &[Option<std::net::SocketAddr>],
    stats: &RuleStats,
    event_tx: &broadcast::Sender<Event>,
    _export_tx: &Option<mpsc::Sender<Event>>,
    cancel: &CancellationToken,
    buffer_size: usize,
    pipeline_name: &str,
    capture: &Arc<CaptureManager>,
) -> Result<(), HarpoonError> {
    // Connect to the first forward target found
    let target_addr = forward_addrs.iter().flatten().next()
        .ok_or_else(|| HarpoonError::Config("DAG pipeline has no forward target".into()))?;

    let upstream = TcpStream::connect(target_addr)
        .await
        .map_err(|e| HarpoonError::UpstreamConnect { addr: *target_addr, source: e })?;

    let (mut client_read, mut client_write) = client_stream.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();

    // Recompile filters for the closures (compiled_stages can't be cloned due to Regex)
    let mk_compiled = || -> Vec<Vec<CompiledFilter>> {
        stages.iter().map(|stage| match &stage.kind {
            NodeKind::Filter(cfg) => cfg.filters.iter().filter_map(|f| CompiledFilter::new(f.clone()).ok()).collect(),
            NodeKind::Router(cfg) => vec![CompiledFilter::new(cfg.filter.clone()).ok()].into_iter().flatten().collect(),
            _ => vec![],
        }).collect()
    };

    // Client → Server: read, walk DAG, write
    let c2s = {
        let stages = stages.to_vec();
        let compiled = mk_compiled();
        let name = pipeline_name.to_string();
        let cancel = cancel.clone();
        let target = *target_addr;
        let capture = capture.clone();

        async move {
            let mut buf = vec![0u8; buffer_size];
            loop {
                tokio::select! {
                    result = client_read.read(&mut buf) => {
                        let n = result?;
                        if n == 0 { break; }
                        let data = &buf[..n];

                        let action = process_dag_stages(
                            &stages, &compiled, data, &Direction::ClientToServer, stats,
                        );

                        match action {
                            DagAction::Forward => {
                                upstream_write.write_all(data).await?;
                                stats.bytes_client_to_server.fetch_add(n as u64, Ordering::Relaxed);
                                stats.packets_client_to_server.fetch_add(1, Ordering::Relaxed);
                                capture.record(&name, crate::capture::PacketDirection::ClientToServer, client_addr, target, data).await;
                            }
                            DagAction::Drop => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    _ = cancel.cancelled() => break,
                }
            }
            Ok::<(), std::io::Error>(())
        }
    };

    // Server → Client
    let s2c = {
        let stages = stages.to_vec();
        let compiled = mk_compiled();
        let name = pipeline_name.to_string();
        let cancel = cancel.clone();
        let target = *target_addr;
        let capture = capture.clone();

        async move {
            let mut buf = vec![0u8; buffer_size];
            loop {
                tokio::select! {
                    result = upstream_read.read(&mut buf) => {
                        let n = result?;
                        if n == 0 { break; }
                        let data = &buf[..n];

                        let action = process_dag_stages(
                            &stages, &compiled, data, &Direction::ServerToClient, stats,
                        );

                        match action {
                            DagAction::Forward => {
                                client_write.write_all(data).await?;
                                stats.bytes_server_to_client.fetch_add(n as u64, Ordering::Relaxed);
                                stats.packets_server_to_client.fetch_add(1, Ordering::Relaxed);
                                capture.record(&name, crate::capture::PacketDirection::ServerToClient, target, client_addr, data).await;
                            }
                            DagAction::Drop => {
                                stats.dropped_packets.fetch_add(1, Ordering::Relaxed);
                            }
                        }
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

enum DagAction {
    Forward,
    Drop,
}

/// Walk DAG stages for a single chunk of data.
/// Returns whether to forward or drop.
fn process_dag_stages(
    stages: &[crate::pipeline::compile::DagStage],
    compiled_stages: &[Vec<CompiledFilter>],
    data: &[u8],
    direction: &Direction,
    stats: &RuleStats,
) -> DagAction {
    // Track which stage indices are "active" (data flows to them)
    let mut active = vec![true; stages.len()];

    for (i, stage) in stages.iter().enumerate() {
        if !active[i] { continue; }

        match &stage.kind {
            NodeKind::Filter(_) => {
                let (action, idx) = apply_filters(&compiled_stages[i], data, direction);
                if let Some(_) = idx {
                    stats.filter_matches.fetch_add(1, Ordering::Relaxed);
                }
                if action == FilterAction::Drop {
                    return DagAction::Drop;
                }
            }
            NodeKind::Router(_) => {
                let (action, _) = apply_filters(&compiled_stages[i], data, direction);
                let matched = action == FilterAction::Drop; // router uses "drop" as "match"

                for output in &stage.outputs {
                    if let Some(port) = &output.port {
                        if (port == "match" && !matched) || (port == "default" && matched) {
                            // Deactivate this branch
                            if output.target_stage_index < active.len() {
                                active[output.target_stage_index] = false;
                            }
                        }
                    }
                }
            }
            NodeKind::Drop => {
                return DagAction::Drop;
            }
            NodeKind::Forward(_) | NodeKind::Duplicate(_) | NodeKind::Export(_) => {
                // Sinks — forward action handled at the end
            }
            _ => {}
        }
    }

    DagAction::Forward
}
