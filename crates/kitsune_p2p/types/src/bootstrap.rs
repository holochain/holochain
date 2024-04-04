//! Types for the bootstrap server
use std::collections::HashSet;
use crate::bin_types::{KitsuneBinType, KitsuneSpace};
use std::sync::Arc;
use crate::tx2::tx2_utils::TxUrl;

/// The number of random agent infos we want to collect from the bootstrap service when we want to
/// populate an empty local space.
/// @todo expose this to network config.
const RANDOM_LIMIT_DEFAULT: u32 = 16;

/// Struct to be encoded for the `random` op.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct RandomQuery {
    /// The space to get random agents from.
    pub space: Arc<KitsuneSpace>,
    /// The maximum number of random agents to retrieve for this query.
    pub limit: RandomLimit,
}

impl Default for RandomQuery {
    fn default() -> Self {
        Self {
            // This is useless, it's here as a placeholder so that ..Default::default() syntax
            // works for limits, not because you'd actually ever want a "default" space.
            space: Arc::new(KitsuneSpace::new(vec![0; 36])),
            limit: RandomLimit::default(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, derive_more::From, derive_more::Into, Clone)]
/// Limit of random peers to return.
pub struct RandomLimit(pub u32);

impl Default for RandomLimit {
    fn default() -> Self {
        Self(RANDOM_LIMIT_DEFAULT)
    }
}

/// The result of storing a new agent info with Kitsune's host.
#[derive(Default, Debug)]
pub struct AgentInfoPut {
    /// URLs that were in the previous agent info for the agent but are no longer present.
    pub removed_urls: HashSet<TxUrl>,
}
