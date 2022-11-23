#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(warnings)]
//! Kitsune P2p Fetch Queue Logic

use kitsune_p2p_types::{dht::region::RegionCoords, KAgent, KOpHash};

mod error;
mod queue;
mod respond;

pub use error::*;
pub use queue::*;
pub use respond::*;

/// Determine what should be fetched.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FetchKey {
    /// Fetch via region.
    Region {
        /// The region coordinates to fetch
        region_coords: RegionCoords,
    },

    /// Fetch via op hash.
    Op {
        /// The hash of the op to fetch.
        op_hash: KOpHash,
    },
}

/// A fetch "unit" that can be de-duplicated.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct FetchRequest {
    /// Description of what to fetch.
    pub key: FetchKey,

    /// If specified, the author of the op.
    /// NOTE: author is additive-only. That is, an op without an author
    /// is the same as one *with* an author, but should be updated to
    /// include the author. It is UB to have two FetchKeys with the
    /// same op_hash, but different authors.
    pub author: Option<KAgent>,

    /// Optional arguments related to fetching the data.
    pub options: Option<FetchOptions>,

    /// Opaque "context" to be provided and interpreted by the host.
    pub context: Option<FetchContext>,
}

impl FetchRequest {
    /// Construct a fetch request with key and context.
    pub fn with_key(key: FetchKey, context: Option<FetchContext>) -> Self {
        Self {
            key,
            author: None,
            context,
            options: Default::default(),
        }
    }

    /// Construct a fetch request with key, context, and author.
    pub fn with_key_and_author(
        key: FetchKey,
        context: Option<FetchContext>,
        author: KAgent,
    ) -> Self {
        Self {
            key,
            author: Some(author),
            context,
            options: Default::default(),
        }
    }
}

/// Options which affect how the fetch is performed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct FetchOptions {
    __: (),
}

/// Usage agnostic context data.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::Deref,
    derive_more::From,
)]
pub struct FetchContext(u32);
