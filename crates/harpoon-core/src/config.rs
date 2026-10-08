use crate::engine::filter::SharedFilterSet;
use crate::types::pipeline::Pipeline;
use crate::types::rule::Rule;

#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub rules: Vec<Rule>,
    pub pipelines: Vec<Pipeline>,
    pub buffer_size: usize,
    pub udp_max_datagram: usize,
    pub shutdown_timeout_secs: u64,
    pub event_channel_capacity: usize,
    pub export_channel_capacity: usize,
    pub tcp_nodelay: bool,
    /// Shared dynamic filter set for linear-tier pipelines (hot-swappable).
    ///
    /// When set, it REPLACES the filters of every rule-derived pipeline (and
    /// of direct Linear pipelines): rules' own `filters` are ignored. The
    /// caller keeps a clone of the [`SharedFilterSet`] and may call `store()`
    /// at any time — running proxies then evaluate traffic against the new
    /// set immediately, with no engine restart and no listener/connection
    /// disruption. Each chunk/datagram is evaluated against the set current
    /// at evaluation time (see [`SharedFilterSet`] docs for granularity).
    ///
    /// Not applied to FastForward (no processing by design) or DAG-tier
    /// pipelines (their own compiled-in filters stay in effect).
    pub shared_filter_set: Option<SharedFilterSet>,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            pipelines: Vec::new(),
            buffer_size: 8192,
            udp_max_datagram: 65507,
            shutdown_timeout_secs: 5,
            event_channel_capacity: 4096,
            export_channel_capacity: 512,
            tcp_nodelay: true,
            shared_filter_set: None,
        }
    }
}
