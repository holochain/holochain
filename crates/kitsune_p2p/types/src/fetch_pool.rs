//! Fetch queue types

/// Info about the fetch queue
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FetchPoolInfo {
    /// Total number of bytes expected to be received through fetches
    pub op_bytes_to_fetch: usize,

    /// Total number of ops expected to be received through fetches
    pub num_ops_to_fetch: usize,
}

/// Gossip has two distinct variants which share a lot of similarities but
/// are fundamentally different and serve different purposes
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
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum GossipType {
    /// The Recent gossip type is aimed at rapidly syncing the most recent
    /// data. It runs frequently and expects frequent diffs at each round.
    Recent,
    /// The Historical gossip type is aimed at comprehensively syncing the
    /// entire common history of two nodes, filling in gaps in the historical
    /// data. It runs less frequently, and expects diffs to be infrequent
    /// at each round.
    Historical,
}
