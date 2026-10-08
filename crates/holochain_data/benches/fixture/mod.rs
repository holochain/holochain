//! Seeded, realistic DHT database fixture for the query benchmarks.

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
