use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::capture::CaptureManager;
use crate::engine::filter::CompiledFilter;
use crate::error::HarpoonError;
use crate::export::sink::run_exporter;
use crate::pipeline::compile::*;
use crate::types::endpoint::Protocol;
use crate::types::event::Event;
use crate::types::rule::UdpSourceMode;
use crate::types::stats::RuleStats;

#[cfg(feature = "tls")]
use crate::tls::cert::CertAuthority;

/// Parameters for a TCP pipeline executor.
pub struct TcpParams {
    pub name: String,
    pub listen_addr: SocketAddr,
    pub target_addr: SocketAddr,
    pub filters: Arc<Vec<CompiledFilter>>,
    pub duplicate_addr: Option<SocketAddr>,
    pub buffer_size: usize,
    pub tcp_nodelay: bool,
    pub capture: Arc<CaptureManager>,
    #[cfg(feature = "tls")]
    pub ca: Option<Arc<CertAuthority>>,
    #[cfg(feature = "tls")]
    pub tls_terminate: bool,
    #[cfg(feature = "tls")]
    pub tls_initiate: bool,
}

/// Parameters for a UDP pipeline executor.
pub struct UdpParams {
    pub name: String,
    pub listen_addr: SocketAddr,
    pub target_addr: SocketAddr,
    pub filters: Arc<Vec<CompiledFilter>>,
    pub duplicate_addr: Option<SocketAddr>,
    pub capture: Arc<CaptureManager>,
    pub udp_source_mode: UdpSourceMode,
    pub idle_timeout_secs: u64,
    pub max_datagram: usize,
}

/// Dispatch an ExecutionPlan to the appropriate executor.
pub fn spawn_plan(
    plan: ExecutionPlan,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    cancel: CancellationToken,
    buffer_size: usize,
    max_datagram: usize,
    tcp_nodelay: bool,
    export_channel_capacity: usize,
    capture: Arc<CaptureManager>,
    #[cfg(feature = "tls")] ca: Option<Arc<CertAuthority>>,
) -> JoinHandle<Result<(), HarpoonError>> {
    match plan {
        ExecutionPlan::FastForward(p) => {
            spawn_fast_forward(
                p, stats, event_tx, cancel, buffer_size, max_datagram,
                tcp_nodelay, export_channel_capacity, capture,
                #[cfg(feature = "tls")] ca,
            )
        }
        ExecutionPlan::Linear(p) => {
            spawn_linear(
                p, stats, event_tx, cancel, buffer_size, max_datagram,
                tcp_nodelay, export_channel_capacity, capture,
                #[cfg(feature = "tls")] ca,
            )
        }
        ExecutionPlan::Dag(p) => {
            spawn_dag(
                p, stats, event_tx, cancel, buffer_size, max_datagram,
                tcp_nodelay, export_channel_capacity, capture,
                #[cfg(feature = "tls")] ca,
            )
        }
    }
}

fn spawn_fast_forward(
    plan: FastForwardPlan,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    cancel: CancellationToken,
    buffer_size: usize,
    max_datagram: usize,
    tcp_nodelay: bool,
    _export_channel_capacity: usize,
    capture: Arc<CaptureManager>,
    #[cfg(feature = "tls")] _ca: Option<Arc<CertAuthority>>,
) -> JoinHandle<Result<(), HarpoonError>> {
    let proto = plan.source.endpoint.protocol;
    match proto {
        Protocol::Tcp => {
            let params = TcpParams {
                name: plan.pipeline_name,
                listen_addr: plan.source.endpoint.addr,
                target_addr: plan.forward.endpoint.addr,
                filters: Arc::new(vec![]),
                duplicate_addr: None,
                buffer_size,
                tcp_nodelay,
                capture: capture.clone(),
                #[cfg(feature = "tls")] ca: None,
                #[cfg(feature = "tls")] tls_terminate: false,
                #[cfg(feature = "tls")] tls_initiate: false,
            };
            tokio::spawn(super::tcp::run_tcp_pipeline(
                params, stats, event_tx, None, cancel,
            ))
        }
        Protocol::Udp => {
            let params = UdpParams {
                name: plan.pipeline_name,
                listen_addr: plan.source.endpoint.addr,
                target_addr: plan.forward.endpoint.addr,
                filters: Arc::new(vec![]),
                duplicate_addr: None,
                capture: capture.clone(),
                udp_source_mode: plan.source.udp_source_mode,
                idle_timeout_secs: plan.source.idle_timeout_secs,
                max_datagram,
            };
            tokio::spawn(super::udp::run_udp_pipeline(
                params, stats, event_tx, None, cancel,
            ))
        }
    }
}

fn spawn_linear(
    plan: LinearPlan,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    cancel: CancellationToken,
    buffer_size: usize,
    max_datagram: usize,
    tcp_nodelay: bool,
    export_channel_capacity: usize,
    capture: Arc<CaptureManager>,
    #[cfg(feature = "tls")] ca: Option<Arc<CertAuthority>>,
) -> JoinHandle<Result<(), HarpoonError>> {
    // Compile filters
    let filters: Vec<CompiledFilter> = plan
        .filters
        .iter()
        .filter_map(|f| CompiledFilter::new(f.clone()).ok())
        .collect();

    // Setup exporter
    let export_tx = plan.exporter.map(|exp_cfg| {
        let (tx, rx) = mpsc::channel(export_channel_capacity);
        tokio::spawn(run_exporter(exp_cfg.kind, rx));
        tx
    });

    let dup_addr = plan.duplicate.map(|d| d.endpoint.addr);
    let proto = plan.source.endpoint.protocol;

    match proto {
        Protocol::Tcp => {
            let params = TcpParams {
                name: plan.pipeline_name,
                listen_addr: plan.source.endpoint.addr,
                target_addr: plan.forward.endpoint.addr,
                filters: Arc::new(filters),
                duplicate_addr: dup_addr,
                buffer_size,
                tcp_nodelay,
                capture: capture.clone(),
                #[cfg(feature = "tls")]
                ca,
                #[cfg(feature = "tls")]
                tls_terminate: plan.tls_terminate.is_some(),
                #[cfg(feature = "tls")]
                tls_initiate: plan.tls_initiate.is_some(),
            };
            tokio::spawn(super::tcp::run_tcp_pipeline(
                params, stats, event_tx, export_tx, cancel,
            ))
        }
        Protocol::Udp => {
            let params = UdpParams {
                name: plan.pipeline_name,
                listen_addr: plan.source.endpoint.addr,
                target_addr: plan.forward.endpoint.addr,
                filters: Arc::new(filters),
                duplicate_addr: dup_addr,
                capture: capture.clone(),
                udp_source_mode: plan.source.udp_source_mode,
                idle_timeout_secs: plan.source.idle_timeout_secs,
                max_datagram,
            };
            tokio::spawn(super::udp::run_udp_pipeline(
                params, stats, event_tx, export_tx, cancel,
            ))
        }
    }
}

fn spawn_dag(
    plan: DagPlan,
    stats: Arc<RuleStats>,
    event_tx: broadcast::Sender<Event>,
    cancel: CancellationToken,
    buffer_size: usize,
    max_datagram: usize,
    tcp_nodelay: bool,
    export_channel_capacity: usize,
    capture: Arc<CaptureManager>,
    #[cfg(feature = "tls")] ca: Option<Arc<CertAuthority>>,
) -> JoinHandle<Result<(), HarpoonError>> {
    let proto = plan.source.endpoint.protocol;

    // Setup exporter if present
    let export_tx = plan.stages.iter().find_map(|s| {
        if let crate::types::pipeline::NodeKind::Export(ref cfg) = s.kind {
            let (tx, rx) = mpsc::channel(export_channel_capacity);
            tokio::spawn(run_exporter(cfg.exporter.kind.clone(), rx));
            Some(tx)
        } else {
            None
        }
    });

    match proto {
        Protocol::Tcp => {
            tokio::spawn(super::dag_executor::run_dag_tcp(
                plan, stats, event_tx, export_tx, cancel,
                buffer_size, tcp_nodelay, capture,
            ))
        }
        Protocol::Udp => {
            // UDP DAG: extract linear components (full DAG for UDP is future)
            let forward = plan.stages.iter().find_map(|s| {
                if let crate::types::pipeline::NodeKind::Forward(ref cfg) = s.kind { Some(cfg.clone()) } else { None }
            });
            let filters: Vec<CompiledFilter> = plan.stages.iter().filter_map(|s| {
                if let crate::types::pipeline::NodeKind::Filter(ref cfg) = s.kind {
                    Some(cfg.filters.iter().filter_map(|f| CompiledFilter::new(f.clone()).ok()).collect::<Vec<_>>())
                } else { None }
            }).flatten().collect();

            match forward {
                Some(fwd) => {
                    let params = UdpParams {
                        name: plan.pipeline_name,
                        listen_addr: plan.source.endpoint.addr,
                        target_addr: fwd.endpoint.addr,
                        filters: Arc::new(filters),
                        duplicate_addr: None,
                        capture,
                        udp_source_mode: plan.source.udp_source_mode,
                        idle_timeout_secs: plan.source.idle_timeout_secs,
                        max_datagram,
                    };
                    tokio::spawn(super::udp::run_udp_pipeline(params, stats, event_tx, export_tx, cancel))
                }
                None => tokio::spawn(async { Ok(()) }),
            }
        }
    }
}
