//! Seeded, realistic DHT database fixture for the query benchmarks.

mod generate;
pub use generate::{generate, Generated, GeneratedChain};

/// Fixture parameters.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct FixtureConfig {
    /// Total number of actions across all authors.
    pub actions: usize,
    /// RNG seed; the same seed always produces the same database.
    pub seed: u64,
}

/// Fixture sizes to benchmark, from `HC_DATA_BENCH_SIZES` or the default
/// `1000,10000,100000`.
#[allow(dead_code)]
pub fn sizes() -> Vec<usize> {
    match std::env::var("HC_DATA_BENCH_SIZES") {
        Ok(v) => v
            .split(',')
            .map(|s| s.trim().parse().expect("HC_DATA_BENCH_SIZES: integer"))
            .collect(),
        Err(_) => vec![1_000, 10_000, 100_000],
    }
}

/// Assert the generator output has the shape the benchmarks rely on.
///
/// Panics with a clear message so a broken generator fails fast at bench
/// start-up instead of producing meaningless numbers.
pub fn check_generated(cfg: FixtureConfig, g: &Generated) {
    let total: usize = g.chains.iter().map(|c| c.records.len()).sum();
    assert!(
        total >= cfg.actions * 95 / 100,
        "generated {total} actions, wanted about {}",
        cfg.actions
    );
    assert!(g.chains.len() >= 4, "need at least 4 authors");
    for chain in &g.chains {
        let mut prev: Option<holo_hash::ActionHash> = None;
        for (i, r) in chain.records.iter().enumerate() {
            assert_eq!(r.action().action_seq(), i as u32, "seq must be dense");
            assert_eq!(r.action().prev_action(), prev.as_ref(), "prev must link");
            prev = Some(r.action_address().clone());
        }
    }
    assert!(g.min_timestamp < g.max_timestamp);
}
