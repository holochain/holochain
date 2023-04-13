//! Types for arbitrary data driven by entropy

use std::sync::Mutex;

use arbitrary::{Arbitrary, Unstructured};
use once_cell::sync::Lazy;

/// 10MB of entropy free for the taking.
/// Useful for initializing arbitrary::Unstructured data
pub static NOISE: Lazy<Vec<u8>> = Lazy::new(|| {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    std::iter::repeat_with(|| rng.gen())
        .take(10_000_000)
        .collect()
});

/// 10MB of random Unstructured data for use with `arbitrary`
pub fn unstructured_noise() -> arbitrary::Unstructured<'static> {
    arbitrary::Unstructured::new(&NOISE)
}

static ENTROPY: Lazy<Mutex<Unstructured<'static>>> =
    Lazy::new(|| Mutex::new(Unstructured::new(&*NOISE)));

/// Additional methods for arbitrary data types
pub trait ArbitraryExt: Arbitrary<'static> {
    /// Generate arbitrary data from built-in noise
    fn fixture() -> Self {
        let mut u = ENTROPY.lock().unwrap();
        if let Ok(a) = Self::arbitrary(&mut u) {
            a
        } else {
            *u = unstructured_noise();
            Self::arbitrary(&mut u).unwrap()
        }
    }
}
impl<T> ArbitraryExt for T where T: Arbitrary<'static> {}
