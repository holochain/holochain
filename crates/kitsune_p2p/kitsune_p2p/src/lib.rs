#![deny(missing_docs)]
//! P2p / dht communication framework.

/// re-exported dependencies
pub mod dependencies {
    pub use ::kitsune_p2p_proxy;
    pub use ::kitsune_p2p_types;
    pub use ::url2;
}

mod types;
pub use types::*;

pub mod gossip;

mod config;
pub use config::*;

mod spawn;
pub use spawn::*;

#[allow(missing_docs)]
#[cfg(any(test, feature = "test_utils"))]
pub mod test_util;

#[cfg(test)]
mod test;

pub mod fixt;

/// 10MB of entropy free for the taking.
/// Useful for initializing arbitrary::Unstructured data
#[cfg(any(test, feature = "test_utils"))]
pub static NOISE: once_cell::sync::Lazy<Vec<u8>> = once_cell::sync::Lazy::new(|| {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    std::iter::repeat_with(|| rng.gen())
        .take(10_000_000)
        .collect()
});
