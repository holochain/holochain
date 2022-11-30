#![deny(missing_docs)]
//! P2p / dht communication framework.
//!
//! ### TLS session key logging
//!
//! To use a tool like wireshark to debug kitsune QUIC communications,
//! enable keylogging via tuning_param:
//!
//! ```
//! # use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
//! # let mut tuning_params = KitsuneP2pTuningParams::default();
//! tuning_params.danger_tls_keylog = "env_keylog".to_string();
//! ```
//!
//! The tuning param by itself will do nothing, you also must specify
//! the file target via the environment variable `SSLKEYLOGFILE`, e.g.:
//!
//! ```no_compile
//! SSLKEYLOGFILE="$(pwd)/keylog" my-kitsune-executable
//! ```
//!
//! As QUIC support within wireshark is in-progress, you'll need a newer
//! version. This documentation was tested with version `3.6.2`.
//!
//! Tell wireshark about your keylog file at:
//!
//! `[Edit] -> [Preferences...] -> [Protocols] -> [TLS] -> [(Pre)-Master-Secret log filename]`
//!
//! Your capture should now include `QUIC` protocol packets, where the
//! `Protected Payload` variants will be able to display internals,
//! such as `STREAM([id])` decrypted content.
//!
//! Also see [https://github.com/quiclog/pcap2qlog](https://github.com/quiclog/pcap2qlog)

/// re-exported dependencies
pub mod dependencies {
    pub use ::kitsune_p2p_proxy;
    pub use ::kitsune_p2p_timestamp;
    pub use ::kitsune_p2p_types;
    pub use ::url2;
}

pub mod metrics;

mod types;
pub use types::*;

pub mod gossip;
pub use gossip::sharded_gossip::GossipDiagnostics;

mod config;
pub use config::*;

mod spawn;
pub use spawn::*;

mod host_api;
pub use host_api::*;

#[allow(missing_docs)]
#[cfg(any(test, feature = "test_utils"))]
pub mod test_util;

#[cfg(any(test, feature = "test_utils"))]
mod test;

pub mod fixt;

/// 10MB of entropy free for the taking.
/// Useful for initializing arbitrary::Unstructured data
#[cfg(any(test, feature = "test_utils"))]
pub static NOISE: once_cell::sync::Lazy<Vec<u8>> = once_cell::sync::Lazy::new(|| {
    use rand::Rng;

    let mut rng = rand::thread_rng();

    // use rand::SeedableRng;
    // let mut rng = rand::rngs::StdRng::seed_from_u64(0);

    std::iter::repeat_with(|| rng.gen())
        .take(10_000_000)
        .collect()
});
