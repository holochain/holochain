//! This doesn't really belong here, but it's the most upstream place to put
//! it without making a new crate

/// 10MB of entropy free for the taking.
/// Useful for initializing arbitrary::Unstructured data
#[cfg(feature = "fuzzing")]
pub static NOISE: once_cell::sync::Lazy<Vec<u8>> = once_cell::sync::Lazy::new(|| {
    use rand::Rng;

    let mut rng = rand::thread_rng();

    // use rand::SeedableRng;
    // let mut rng = rand::rngs::StdRng::seed_from_u64(0);

    std::iter::repeat_with(|| rng.gen())
        .take(10_000_000)
        .collect()
});
