//! Types for warrants
use holo_hash::ActionHash;
pub use holochain_serialized_bytes::prelude::*;

use crate::signature::Signed;

/// Placeholder for warrant type
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash, derive_more::From,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum Warrant {
    /// Signifies evidence of a breach of chain integrity
    ChainIntegrity(ChainIntegrityWarrant),
}

/// A warrant which is sent to AgentActivity authorities
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum ChainIntegrityWarrant {
    /// Something invalid was authored on a chain, according to
    /// a specific op type.
    /// When we receive this warrant, we validate the data as the specified type of Op.
    InvalidChainOp(ActionHash, /* DhtOpType, */ ValidationType),

    /// Proof of chain fork.
    ChainFork(ActionHash, ActionHash),
}

/// Not necessary but nice to have
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash, derive_more::Display,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum ValidationType {
    /// Sys validation
    Sys,
    /// App validation
    App,
}

/// A signed warrant
pub type SignedWarrant = Signed<Warrant>;
