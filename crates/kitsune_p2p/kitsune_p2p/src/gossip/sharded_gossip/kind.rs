//! Defines the ways of referring to the two types of gossip: Recent and Historical

/// Gossip has two distinct variants which share a lot of similarities but
/// are fundamentally different and serve different purposes
pub trait GossipKind: std::fmt::Debug + Clone + Send + Sync + 'static {
    /// Get the enum form of this type
    fn gossip_type() -> GossipType;
}

/// The enum counterpart to GossipKind
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Debug,
    derive_more::Display,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum GossipType {
    /// Recent
    Recent,
    /// Historical
    Historical,
}

/// The Recent gossip type is aimed at rapidly syncing the most recent
/// data. It runs frequently and expects frequent diffs at each round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recent;

/// The Historical gossip type is aimed at comprehensively syncing the
/// entire common history of two nodes, filling in gaps in the historical
/// data. It runs less frequently, and expects diffs to be infrequent
/// at each round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Historical;

impl GossipKind for Recent {
    fn gossip_type() -> GossipType {
        GossipType::Recent
    }
}
impl GossipKind for Historical {
    fn gossip_type() -> GossipType {
        GossipType::Historical
    }
}
