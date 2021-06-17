#![deny(missing_docs)]
//! Proxy transport module for kitsune-p2p

use derive_more::*;
use futures::future::FutureExt;
use ghost_actor::dependencies::must_future::MustBoxFuture;
use ghost_actor::GhostControlSender;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::url2;
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::transport::*;
use lair_keystore_api::actor::*;
use std::sync::Arc;

pub(crate) fn blake2b_32(data: &[u8]) -> Vec<u8> {
    blake2b_simd::Params::new()
        .hash_length(32)
        .to_state()
        .update(data)
        .finalize()
        .as_bytes()
        .to_vec()
}

pub mod tx2;

mod proxy_url;
pub use proxy_url::*;

pub mod wire;
pub(crate) use wire::*;

mod wire_read;
mod wire_write;

mod tls_cli;
mod tls_srv;

#[cfg(test)]
mod tls_tests;

mod inner_listen;
pub use inner_listen::*;

mod config;
pub use config::*;
