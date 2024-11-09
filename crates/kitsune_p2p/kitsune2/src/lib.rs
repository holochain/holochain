//! Kitsune2 p2p / dht communication framework.
//!
//! Note: Some types test types and functions are included in the public api
//!       of this crate. They are included because they are useful for testing
//!       by anyone implementing Kitsune2, and rust is good at compiling
//!       out code that is not used in the final executable.
#![deny(missing_docs)]

use std::collections::HashMap;
use std::io::Result;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::future::BoxFuture;
use kitsune2_api::*;

/// Extend the api Builder with additional concrete functions.
pub trait BuilderExt {
    /// Create a default test builder.
    fn create_test() -> kitsune2_api::builder::Builder;

    #[cfg(test)]
    /// A test-api to generate a PeerStore instance from this builder.
    fn create_peer_store(&self) -> BoxFuture<'_, Result<kitsune2_api::peer_store::DynPeerStore>>;
}

impl BuilderExt for kitsune2_api::builder::Builder {
    fn create_test() -> kitsune2_api::builder::Builder {
        kitsune2_api::builder::Builder {
            peer_store: factories::MemPeerStoreFactory::create(),
        }
    }

    #[cfg(test)]
    fn create_peer_store(&self) -> BoxFuture<'_, Result<kitsune2_api::peer_store::DynPeerStore>> {
        let b = std::sync::Arc::new(self.clone());
        self.peer_store.create(b)
    }
}

mod test_agent;
pub use test_agent::*;

pub mod factories;
