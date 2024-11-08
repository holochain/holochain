//! Kitsune2 p2p / dht communication framework api.
#![deny(missing_docs)]

use std::io::Result;
use std::sync::Arc;

use bytes::Bytes;
use futures::future::BoxFuture;

/// A Kitsune2 identifier.
/// Display and Debug should both return the canonical string representation
/// of this identifier. For Holochain, that means the base64 multihash repr.
pub trait Id: 'static + Send + Sync + std::fmt::Debug + std::fmt::Display {
    /// The raw bytes of the identifier.
    /// Note, in the case of legacy kitsune, this should only return
    /// the raw 32 bytes of the pub_key or direct hash. It should not
    /// return any type prefix or location suffix.
    fn bytes(&self) -> Bytes;

    /// The precalculated location u32 based on the id.
    /// Any required validation should not happen in this call,
    /// but should have been performed before this instance was constructed.
    fn loc(&self) -> u32;
}

/// Trait-object [Id].
pub type DynId = Arc<dyn Id>;

mod timestamp;
pub use timestamp::*;

pub mod agent;
pub mod arq;
pub mod builder;
pub mod peer_store;
