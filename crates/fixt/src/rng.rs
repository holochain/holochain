use parking_lot::Mutex;
use rand::SeedableRng;
use rand::{rngs::StdRng, RngCore};
use std::sync::Arc;

lazy_static::lazy_static! {
    /// The key to access the ChainEntries databases
    pub static ref FIXTURATOR_RNG: FixtRng = {
        let seed: u64 = match std::env::var("FIXTURATOR_SEED") {
            Ok(seed_str) => {
                seed_str.parse().expect("Expected integer for FIXTURATOR_SEED")
            }
            Err(std::env::VarError::NotPresent) => { rand::random() },
            Err(std::env::VarError::NotUnicode(v)) => { panic!("Invalid FIXTURATOR_SEED value: {:?}", v) },
        };
        println!("Fixturator seed: {}", seed);
        FixtRng(Arc::new(
            Mutex::new(StdRng::seed_from_u64(seed))
        ))
    };
}

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

pub fn random<T>() -> T
where
    rand::distributions::Standard: rand::distributions::Distribution<T>,
{
    use rand::Rng;
    crate::rng().gen()
}

pub fn rng() -> FixtRng {
    FIXTURATOR_RNG.clone()
}

// fn _with_rng<F, T>(f: F) -> T
// where
//     F: FnOnce(&mut StdRng) -> T,
// {
//     let mut rng = crate::rng();
//     f(&mut rng)
// }
