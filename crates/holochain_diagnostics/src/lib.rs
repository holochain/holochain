pub use holochain;

pub use rand::rngs::StdRng;
pub use rand::Rng;
use rand::*;

/// A RNG suitable for testing, if no seed is passed, uses standard random seed.
pub fn seeded_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| thread_rng().gen());
    tracing::info!("RNG seed: {}", seed);
    StdRng::seed_from_u64(seed)
}
