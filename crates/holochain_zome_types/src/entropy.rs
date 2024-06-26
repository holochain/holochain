//! Types for arbitrary data driven by entropy

use contrafact::Generator;
use rand::{rngs::StdRng, Rng, SeedableRng};

#[cfg(feature = "fuzzing")]
pub use kitsune_p2p_timestamp::noise::NOISE;

fn seeded_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| rand::thread_rng().gen());
    StdRng::seed_from_u64(seed)
}

/// Get some noise to feed into arbitrary::Unstructured
pub fn noise(seed: Option<u64>, size: usize) -> Vec<u8> {
    let mut rng = seeded_rng(seed);
    std::iter::repeat_with(|| rng.gen()).take(size).collect()
}

/// Alias for arbitrary::Unstructured::new
pub fn unstructured(noise: &[u8]) -> arbitrary::Unstructured<'_> {
    arbitrary::Unstructured::new(noise)
}

/// 10MB of random Unstructured data for use with `arbitrary`
pub fn unstructured_noise() -> arbitrary::Unstructured<'static> {
    arbitrary::Unstructured::new(&NOISE)
}

/// A Generator backed by unstructured_noise
pub fn random_generator() -> Generator<'static> {
    Generator::from(unstructured_noise())
}
