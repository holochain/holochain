//! Types for warrants
pub use holochain_serialized_bytes::prelude::*;

/// Placeholder for warrant type
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializedBytes,
    Eq,
    PartialEq,
    Hash,
    derive_more::Display,
    derive_more::From,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct Warrant;
