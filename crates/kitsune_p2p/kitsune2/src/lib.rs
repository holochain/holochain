//! Kitsune2 p2p / dht communication framework.
#![deny(missing_docs)]

use std::collections::HashMap;
use std::io::Result;
use std::sync::{Arc, Mutex};

use kitsune2_api::*;
use bytes::Bytes;
use futures::future::BoxFuture;

/// Extend the api Builder with additional concrete functions.
pub trait BuilderExt {
    /// Create a default test builder.
    fn create_test() -> kitsune2_api::builder::Builder;

    #[cfg(test)]
    /// A test-api to generate a PeerStore instance from this builder.
    fn create_peer_store(&self) -> kitsune2_api::peer_store::DynPeerStore;
}

impl BuilderExt for kitsune2_api::builder::Builder {
    fn create_test() -> kitsune2_api::builder::Builder {
        kitsune2_api::builder::Builder {
            peer_store: factories::MemPeerStoreFactory::create(),
        }
    }

    #[cfg(test)]
    fn create_peer_store(&self) -> kitsune2_api::peer_store::DynPeerStore {
        let b = std::sync::Arc::new(self.clone());
        self.peer_store.create(b)
    }
}

pub mod factories;
