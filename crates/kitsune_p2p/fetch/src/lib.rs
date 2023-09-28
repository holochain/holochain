#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(warnings)]

//! Kitsune P2p Fetch Queue Logic

use kitsune_p2p_types::{KAgent, KOpHash, KSpace};

mod pool;
mod respond;
mod rough_sized;

#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;

pub use pool::*;
pub use respond::*;
pub use rough_sized::*;

/// Determine what should be fetched.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
#[serde(tag = "type", content = "key", rename_all = "camelCase")]
pub enum FetchKey {
    /// Fetch via op hash.
    Op(KOpHash),
}

/// A fetch "unit" that can be de-duplicated.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct FetchPoolPush {
    /// Description of what to fetch.
    pub key: FetchKey,

    /// The space this op belongs to
    pub space: KSpace,

    /// The source to fetch the op from
    pub source: FetchSource,

    /// The approximate size of the item
    pub size: Option<RoughInt>,

    /// If specified, the author of the op.
    /// NOTE: author is additive-only. That is, an op without an author
    /// is the same as one *with* an author, but should be updated to
    /// include the author. It is UB to have two FetchKeys with the
    /// same op_hash, but different authors.
    pub author: Option<KAgent>,

    /// Opaque "context" to be provided and interpreted by the host.
    pub context: Option<FetchContext>,
}

/// Usage agnostic context data.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::Deref,
    derive_more::From,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct FetchContext(pub u32);
