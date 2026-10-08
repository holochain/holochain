//! Benchmark groups, one per runtime flow.

pub mod cascade_local;
// pub mod authority;
// pub mod gossip;
// pub mod workflow;
pub mod zome_call;

use std::sync::atomic::{AtomicUsize, Ordering};

/// Rotates through sampled keys so consecutive iterations hit different
/// rows instead of one cached page.
pub struct Rotate<'a, T> {
    items: &'a [T],
    next: AtomicUsize,
}

impl<'a, T> Rotate<'a, T> {
    pub fn new(items: &'a [T]) -> Self {
        assert!(!items.is_empty(), "no sample keys for this benchmark");
        Self {
            items,
            next: AtomicUsize::new(0),
        }
    }

    pub fn next(&self) -> &'a T {
        let i = self.next.fetch_add(1, Ordering::Relaxed);
        &self.items[i % self.items.len()]
    }
}

/// `criterion` group settings shared by every flow group.
pub fn configure(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    group.sample_size(30);
    group.warm_up_time(std::time::Duration::from_secs(1));
    group.measurement_time(std::time::Duration::from_secs(5));
}
