//! Seedable random number generator to be used in all fixturator randomness
//!
//! In tests, when an unpredictable value causes a test failure, it's important to
//! be able to re-run the test with the same values. This module provides a RNG
//! whose seed will be set automatically and printed to stdout before each test run.
//! To use a previous seed, just set the FIXT_SEED environment variable to the value
//! of a previous run'

use parking_lot::Mutex;
use rand::rngs::StdRng;
use rand::RngCore;
use rand::SeedableRng;
use std::sync::Arc;

lazy_static::lazy_static! {
    /// The singleton global RNG for test randomness
    static ref FIXT_RNG: FixtRng = {
        let seed: u64 = match std::env::var("FIXT_SEED") {
            Ok(seed_str) => {
                seed_str.parse().expect("Expected integer for FIXT_SEED")
            }
            Err(std::env::VarError::NotPresent) => { rand::random() },
            Err(std::env::VarError::NotUnicode(v)) => { panic!("Invalid FIXT_SEED value: {:?}", v) },
        };
        println!("Fixturator seed: {}", seed);
        FixtRng(Arc::new(
            Mutex::new(StdRng::seed_from_u64(seed))
        ))
    };
}

/// A seedable RNG which uses an Arc and a Mutex to allow easy cloneability and thread safety.
/// A singleton global instance is created in this module. See module-level docs for more info.
#[derive(Clone)]
pub struct FixtRng(Arc<Mutex<StdRng>>);

impl RngCore for FixtRng {
    fn next_u32(&mut self) -> u32 {
        self.0.lock().next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.lock().next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.lock().fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.0.lock().try_fill_bytes(dest)
    }
}

/// Access the seeded random number generator. This should be used in all places where
/// tests produce random values.
pub fn rng() -> FixtRng {
    FIXT_RNG.clone()
}
