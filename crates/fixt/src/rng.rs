use parking_lot::{Mutex, MutexGuard};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::sync::Arc;

lazy_static::lazy_static! {
    /// The key to access the ChainEntries databaseS
    pub static ref FIXTURATOR_RNG: Mutex<StdRng> = {
        let seed: u64 = match std::env::var("FIXTURATOR_SEED") {
            Ok(seed_str) => {
                seed_str.parse().expect("Expected integer for FIXTURATOR_SEED")
            }
            Err(std::env::VarError::NotPresent) => { rand::random() },
            Err(std::env::VarError::NotUnicode(v)) => { panic!("Invalid FIXTURATOR_SEED value: {:?}", v) },
        };
        println!("Fixturator seed: {}", seed);
        // Arc::new(
            Mutex::new(StdRng::seed_from_u64(seed))
        // );
    };
}

// pub struct FixtRng(Arc)

pub fn random<T>() -> T
where
    rand::distributions::Standard: rand::distributions::Distribution<T>,
{
    use rand::Rng;
    (*crate::rng()).gen()
}

pub fn rng<'a>() -> MutexGuard<'a, StdRng> {
    FIXTURATOR_RNG.lock()
}

fn _with_rng<F, T>(f: F) -> T
where
    F: FnOnce(&mut StdRng) -> T,
{
    let mut rng = crate::rng();
    f(&mut *rng)
}
