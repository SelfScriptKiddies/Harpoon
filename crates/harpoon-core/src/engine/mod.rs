pub mod dag_executor;
pub mod executor;
pub mod filter;
#[cfg(feature = "http2")]
pub mod http2;
pub mod tcp;
pub mod udp;
#[cfg(feature = "transparent-udp")]
pub mod udp_transparent;

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::config::CoreConfig;
use crate::error::HarpoonError;
use crate::pipeline::compat::rule_to_pipeline;
use crate::pipeline::compile;
use crate::types::event::Event;
use crate::types::pipeline::Pipeline;
use crate::types::stats::{RuleStats, RuleStatsSnapshot};

#[cfg(feature = "tls")]
use crate::tls::cert::CertAuthority;

pub struct EngineHandle {
    cancel: CancellationToken,
    event_tx: broadcast::Sender<Event>,
    stats: Vec<(String, Arc<RuleStats>)>,
    join_handles: Vec<JoinHandle<Result<(), HarpoonError>>>,
    capture: Arc<crate::capture::CaptureManager>,
    metrics: Arc<crate::capture::metrics::MetricsCollector>,
}

impl EngineHandle {
    pub fn stop(&self) {
        self.cancel.cancel();
    }

    pub async fn shutdown(self) -> Vec<Result<(), HarpoonError>> {
        self.cancel.cancel();

        let timeout = tokio::time::Duration::from_secs(5);
        let mut results = Vec::new();

        for handle in self.join_handles {
            match tokio::time::timeout(timeout, handle).await {
                Ok(Ok(r)) => results.push(r),
                Ok(Err(e)) => results.push(Err(HarpoonError::Config(format!("task panic: {e}")))),
                Err(_) => results.push(Err(HarpoonError::Shutdown)),
            }
        }

        results
    }

    pub fn stats_snapshot(&self) -> Vec<RuleStatsSnapshot> {
        self.stats
            .iter()
            .map(|(name, s)| s.snapshot(name.clone()))
            .collect()
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    pub fn capture_manager(&self) -> &Arc<crate::capture::CaptureManager> {
        &self.capture
    }

    pub fn metrics_collector(&self) -> &Arc<crate::capture::metrics::MetricsCollector> {
        &self.metrics
    }
}

pub async fn run(config: CoreConfig) -> Result<EngineHandle, HarpoonError> {
    run_with_capture(config, crate::capture::CaptureManager::new()).await
}

pub async fn run_with_capture(
    config: CoreConfig,
    capture: std::sync::Arc<crate::capture::CaptureManager>,
) -> Result<EngineHandle, HarpoonError> {
    let cancel = CancellationToken::new();
    let (event_tx, _) = broadcast::channel(config.event_channel_capacity);
    let mut handles: Vec<JoinHandle<Result<(), HarpoonError>>> = Vec::new();
    let mut stats_vec = Vec::new();

    // 1. Convert rules to pipelines + append direct pipelines
    let mut all_pipelines: Vec<Pipeline> = config
        .rules
        .iter()
        .map(rule_to_pipeline)
        .collect();
    all_pipelines.extend(config.pipelines.clone());

    // 2. Compile each pipeline into an ExecutionPlan
    let mut plans = Vec::new();
    for p in all_pipelines {
        let plan = compile::compile(p.clone()).map_err(|e| HarpoonError::PipelineCompile {
            pipeline: p.name.clone(),
            detail: format!("{e}"),
        })?;
        plans.push(plan);
    }

    // 3. Build CA if any pipeline needs TLS
    #[cfg(feature = "tls")]
    let ca: Option<Arc<CertAuthority>> = build_ca_from_plans(&plans)?;

    // 4. Dispatch each plan to the appropriate executor
    for plan in plans {
        let pipeline_stats = Arc::new(RuleStats::default());
        let pipeline_name = plan.name().to_string();
        stats_vec.push((pipeline_name, pipeline_stats.clone()));

        let h = executor::spawn_plan(
            plan,
            pipeline_stats,
            event_tx.clone(),
            cancel.child_token(),
            config.buffer_size,
            config.udp_max_datagram,
            config.tcp_nodelay,
            config.export_channel_capacity,
            capture.clone(),
            #[cfg(feature = "tls")]
            ca.clone(),
        );
        handles.push(h);
    }

    // Spawn metrics collector
    let metrics = crate::capture::metrics::MetricsCollector::new();
    crate::capture::metrics::spawn_metrics_collector(
        metrics.clone(),
        stats_vec.clone(),
        cancel.child_token(),
    );

    Ok(EngineHandle {
        cancel,
        event_tx,
        stats: stats_vec,
        join_handles: handles,
        capture,
        metrics,
    })
}

#[cfg(feature = "tls")]
fn build_ca_from_plans(
    plans: &[compile::ExecutionPlan],
) -> Result<Option<Arc<CertAuthority>>, HarpoonError> {
    for plan in plans {
        if let compile::ExecutionPlan::Linear(ref lp) = plan {
            if let Some(ref tls_cfg) = lp.tls_terminate {
                let ca =
                    CertAuthority::from_pem_files(&tls_cfg.ca_cert_path, &tls_cfg.ca_key_path)?;
                return Ok(Some(Arc::new(ca)));
            }
        }
    }
    Ok(None)
}
