use kitsune_p2p_types::{dht::region::RegionCoords, KAgent, KOpHash};

mod error;
mod queue;

pub use error::*;
pub use queue::*;

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
    pub fn with_key(key: FetchKey, context: Option<FetchContext>) -> Self {
        Self {
            key,
            author: None,
            context,
            options: Default::default(),
        }
    }

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

/// Options which affect how the fetch is performed
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct FetchOptions {
    __: (),
}

pub struct FetchResponse {
    _op_data: Vec<()>,
}

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
