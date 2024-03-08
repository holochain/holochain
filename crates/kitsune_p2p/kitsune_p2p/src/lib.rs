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
    pub use ::kitsune_p2p_fetch;
    pub use ::kitsune_p2p_proxy;
    pub use ::kitsune_p2p_timestamp;
    pub use ::kitsune_p2p_types;
    pub use ::url2;
}

/// This value determines protocol compatibility.
/// Any time there is a protocol breaking change, this number must be incremented.
pub const KITSUNE_PROTOCOL_VERSION: u16 = 0;

pub mod metrics;

mod types;
pub use types::*;

pub mod gossip;
pub use gossip::sharded_gossip::KitsuneDiagnostics;

mod spawn;
pub use spawn::*;

mod host_api;
pub use host_api::*;

pub use meta_net::PreflightUserData;

#[allow(missing_docs)]
#[cfg(feature = "test_utils")]
pub mod test_util;

#[cfg(test)]
mod test;

#[cfg(feature = "fuzzing")]
pub use kitsune_p2p_timestamp::noise::NOISE;
