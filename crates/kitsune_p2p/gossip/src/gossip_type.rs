#[derive(Clone, Debug, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
/// The type of gossip module running this gossip.
pub enum GossipModuleType {
    /// Recent sharded gossip.
    ShardedRecent,
    /// Historical sharded gossip.
    ShardedHistorical,
}

/// Gossip has two distinct variants which share a lot of similarities but
/// are fundamentally different and serve different purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl std::fmt::Display for GossipType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GossipType::Recent => write!(f, "recent"),
            GossipType::Historical => write!(f, "historical"),
        }
    }
}

impl From<GossipType> for GossipModuleType {
    fn from(g: GossipType) -> Self {
        match g {
            GossipType::Recent => GossipModuleType::ShardedRecent,
            GossipType::Historical => GossipModuleType::ShardedHistorical,
        }
    }
}
