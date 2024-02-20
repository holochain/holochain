#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(warnings)]

//! Kitsune P2p Fetch Queue Logic

use kitsune_p2p_types::{KOpHash, KSpace};

mod backoff;
mod pool;
mod queue;
mod respond;
mod rough_sized;
mod source;

#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;

pub use pool::*;
pub use respond::*;
pub use rough_sized::*;
pub use source::FetchSource;

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
pub struct FetchPoolPush {
    /// Description of what to fetch.
    pub key: FetchKey,

    /// The space this op belongs to
    pub space: KSpace,

    /// The source to fetch the op from
    pub source: FetchSource,

    /// The means by which this hash arrived, either via Publish or Gossip.
    pub transfer_method: TransferMethod,

    /// The approximate size of the item
    pub size: Option<RoughInt>,

    /// Opaque "context" to be provided and interpreted by the host.
    pub context: Option<FetchContext>,
}

/// The possible methods of transferring op hashes
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum TransferMethod {
    /// Transfer by publishing
    Publish,
    /// Transfer by gossiping
    Gossip,
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
