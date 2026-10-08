use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::error::HarpoonError;
use crate::types::filter::{Direction, Filter, FilterAction, FilterKind};

#[derive(Debug)]
pub struct CompiledFilter {
    pub filter: Filter,
    #[cfg(feature = "regex-filter")]
    compiled_regex: Option<regex::bytes::Regex>,
}

impl CompiledFilter {
    pub fn new(filter: Filter) -> Result<Self, HarpoonError> {
        #[cfg(feature = "regex-filter")]
        let compiled_regex = match &filter.kind {
            FilterKind::Regex(pattern) => {
                if pattern.len() > 1024 {
                    return Err(HarpoonError::Filter(
                        "regex pattern too long (max 1024 bytes)".into(),
                    ));
                }
                let re = regex::bytes::RegexBuilder::new(pattern)
                    .size_limit(1 << 20) // 1 MB compiled size limit
                    .build()
                    .map_err(|e| HarpoonError::Filter(format!("invalid regex: {e}")))?;
                Some(re)
            }
            _ => None,
        };

        Ok(Self {
            filter,
            #[cfg(feature = "regex-filter")]
            compiled_regex,
        })
    }

    pub fn matches(&self, data: &[u8]) -> bool {
        match &self.filter.kind {
            FilterKind::Substr(s) => {
                let pattern = s.as_bytes();
                data.windows(pattern.len()).any(|w| w == pattern)
            }
            FilterKind::BinarySubstr(pattern) => {
                data.windows(pattern.len()).any(|w| w == pattern.as_slice())
            }
            #[cfg(feature = "regex-filter")]
            FilterKind::Regex(_) => self
                .compiled_regex
                .as_ref()
                .map(|re| re.is_match(data))
                .unwrap_or(false),
        }
    }

    pub fn applies_to_direction(&self, direction: &Direction) -> bool {
        match &self.filter.direction {
            Direction::Both => true,
            d => d == direction,
        }
    }
}

pub fn apply_filters(
    filters: &[CompiledFilter],
    data: &[u8],
    direction: &Direction,
) -> (FilterAction, Option<usize>) {
    for (i, f) in filters.iter().enumerate() {
        if !f.applies_to_direction(direction) {
            continue;
        }
        if f.matches(data) {
            return (f.filter.action_on_match.clone(), Some(i));
        }
    }
    (FilterAction::Pass, None)
}

/// A runtime-swappable set of compiled filters, shared by all pipelines that
/// reference it.
///
/// Purpose: hot-swap the effective filter set (e.g. a control plane syncing
/// block lists) WITHOUT restarting the engine — listeners stay bound, running
/// connections keep flowing, and traffic evaluated after the swap is checked
/// against the new set.
///
/// Concurrency guarantees:
/// - [`SharedFilterSet::store`] is a single atomic pointer swap (`arc-swap`):
///   readers on the traffic hot path never block and never see a lock;
/// - every evaluated chunk sees exactly one consistent snapshot — the old or
///   the new set, never a mix, and never "no filters apply".
///
/// Granularity (documented semantics): each chunk / datagram is evaluated
/// against the set that is current *at evaluation time*. Connections opened
/// before a swap pick up the new set on their next chunk — i.e. an existing
/// connection sending newly-blocked content is dropped mid-connection
/// (`drop_connection` breaks it, `drop` suppresses the data). A chunk already
/// past its filter evaluation during a swap completes under the old set,
/// which is the safe direction: a brief window of OLD filters, never none.
pub struct SharedFilterSet(Arc<ArcSwap<Vec<CompiledFilter>>>);

impl SharedFilterSet {
    pub fn new(filters: Vec<CompiledFilter>) -> Self {
        Self(Arc::new(ArcSwap::from_pointee(filters)))
    }

    /// A set with no filters — proxies pass all traffic until a `store`.
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Atomically replace the whole filter set. Takes effect for traffic
    /// evaluated after the swap; does not interrupt listeners or connections.
    pub fn store(&self, filters: Vec<CompiledFilter>) {
        self.0.store(Arc::new(filters));
    }

    /// Snapshot of the currently effective set. Lock-free; cheap Arc clone.
    /// Held only for the duration of one chunk evaluation.
    pub fn current(&self) -> Arc<Vec<CompiledFilter>> {
        self.0.load_full()
    }

    pub fn len(&self) -> usize {
        self.0.load_full().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Clone for SharedFilterSet {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for SharedFilterSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedFilterSet")
            .field("len", &self.len())
            .finish()
    }
}

/// The filter set a pipeline evaluates traffic against.
///
/// - [`FilterView::Static`]: classic behavior — filters compiled once at
///   engine start and immutable for the engine's lifetime (the owned
///   `Rule.filters` path; all pre-existing semantics unchanged).
/// - [`FilterView::Shared`]: backed by a [`SharedFilterSet`] that may be
///   swapped at any moment; the current set is loaded per evaluated chunk.
#[derive(Debug, Clone)]
pub enum FilterView {
    Static(Arc<Vec<CompiledFilter>>),
    Shared(SharedFilterSet),
}

impl FilterView {
    pub fn empty() -> Self {
        Self::Static(Arc::new(Vec::new()))
    }

    /// True only for a `Static` set that is empty.
    ///
    /// A `Shared` set must NEVER qualify for the zero-copy fast path: it can
    /// become non-empty at any time via a hot-swap, and a connection sent
    /// down the filterless fast path can never be filtered again.
    pub fn is_static_empty(&self) -> bool {
        matches!(self, Self::Static(v) if v.is_empty())
    }

    /// Snapshot of the currently effective filter set (cheap Arc clone).
    /// `Static` returns the immutable set; `Shared` loads the current one.
    pub fn current(&self) -> Arc<Vec<CompiledFilter>> {
        match self {
            Self::Static(v) => v.clone(),
            Self::Shared(s) => s.current(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop_filter(pattern: &str) -> Filter {
        Filter {
            kind: FilterKind::Substr(pattern.into()),
            direction: Direction::Both,
            action_on_match: FilterAction::Drop,
        }
    }

    #[test]
    fn test_shared_filter_set_store_and_current() {
        let set = SharedFilterSet::empty();
        assert!(set.is_empty());

        set.store(vec![CompiledFilter::new(drop_filter("bad")).unwrap()]);
        assert_eq!(set.len(), 1);
        let current = set.current();
        assert_eq!(current.len(), 1);
        let (action, idx) = apply_filters(&current, b"has bad bytes", &Direction::Both);
        assert_eq!(action, FilterAction::Drop);
        assert_eq!(idx, Some(0));

        // Swap back to empty: readers immediately see the new set.
        set.store(Vec::new());
        assert!(set.is_empty());
        let (action, idx) = apply_filters(&set.current(), b"has bad bytes", &Direction::Both);
        assert_eq!(action, FilterAction::Pass);
        assert_eq!(idx, None);

        // The pre-swap snapshot is unaffected (immutable Arc).
        assert_eq!(current.len(), 1);
    }

    #[test]
    fn test_shared_filter_set_snapshot_stable_across_swap() {
        // A writer swapping concurrently must never tear a reader's snapshot:
        // every load yields a complete old-or-new set.
        let set = SharedFilterSet::new(vec![CompiledFilter::new(drop_filter("a")).unwrap()]);
        let first = set.current();
        set.store(vec![
            CompiledFilter::new(drop_filter("b")).unwrap(),
            CompiledFilter::new(drop_filter("c")).unwrap(),
        ]);
        let second = set.current();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 2);
        // Clones share the same underlying set.
        let clone = set.clone();
        assert_eq!(clone.len(), 2);
    }

    #[test]
    fn test_filter_view_static_vs_shared() {
        let static_view = FilterView::empty();
        assert!(static_view.is_static_empty());

        let shared_view = FilterView::Shared(SharedFilterSet::empty());
        // Shared set is empty NOW, but must not be treated as statically empty.
        assert!(!shared_view.is_static_empty());
        assert!(shared_view.current().is_empty());
    }
}
