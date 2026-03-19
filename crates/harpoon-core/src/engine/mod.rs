pub mod filter;
pub mod tcp;
pub mod udp;

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::config::CoreConfig;
use crate::error::HarpoonError;
use crate::export::sink::run_exporter;
use crate::types::endpoint::Protocol;
use crate::types::event::Event;
use crate::types::stats::{RuleStats, RuleStatsSnapshot};

use self::filter::CompiledFilter;

pub struct EngineHandle {
    cancel: CancellationToken,
    event_tx: broadcast::Sender<Event>,
    stats: Vec<(String, Arc<RuleStats>)>,
    join_handles: Vec<JoinHandle<Result<(), HarpoonError>>>,
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
}

pub async fn run(config: CoreConfig) -> Result<EngineHandle, HarpoonError> {
    let cancel = CancellationToken::new();
    let (event_tx, _event_rx) = broadcast::channel(1024);
    let mut handles: Vec<JoinHandle<Result<(), HarpoonError>>> = Vec::new();
    let mut stats_vec = Vec::new();

    for rule in config.rules {
        let rule_stats = Arc::new(RuleStats::default());
        stats_vec.push((rule.name.clone(), rule_stats.clone()));

        // Compile filters
        let compiled_filters: Vec<CompiledFilter> = rule
            .filters
            .iter()
            .map(|f| CompiledFilter::new(f.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let filters = Arc::new(compiled_filters);

        // Setup exporter if configured
        let export_tx = if let Some(ref exp_cfg) = rule.exporter {
            let (tx, rx) = mpsc::channel(256);
            let kind = exp_cfg.kind.clone();
            tokio::spawn(run_exporter(kind, rx));
            Some(tx)
        } else {
            None
        };

        match rule.listen.protocol {
            Protocol::Tcp => {
                let h = tokio::spawn(tcp::run_tcp_rule(
                    rule,
                    rule_stats,
                    filters,
                    event_tx.clone(),
                    export_tx,
                    cancel.child_token(),
                    config.buffer_size,
                ));
                handles.push(h);
            }
            Protocol::Udp => {
                let h = tokio::spawn(udp::run_udp_rule(
                    rule,
                    rule_stats,
                    filters,
                    event_tx.clone(),
                    export_tx,
                    cancel.child_token(),
                    config.udp_max_datagram,
                ));
                handles.push(h);
            }
        }
    }

    Ok(EngineHandle {
        cancel,
        event_tx,
        stats: stats_vec,
        join_handles: handles,
    })
}
