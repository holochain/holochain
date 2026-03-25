//! # Source Chain Filtering
//! Types for filtering the source chain.

use crate::MigrationTarget;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;

#[cfg(test)]
mod test;

/// Filter the contiguous chain of actions from an agent's source chain.
///
/// Starting from a specified `chain_top` [`ActionHash`], the filter walks backwards through
/// the linear chain path, towards genesis, excluding any forked actions, and bounded by
/// the specified [`LimitConditions`].
#[derive(Serialize, Deserialize, SerializedBytes, Debug, PartialEq, Eq, Hash, Clone)]
pub struct ChainFilter<H: Eq + Ord + std::hash::Hash = ActionHash> {
    /// The starting position of the filter.
    pub chain_top: H,
    /// The limit conditions used by this filter.
    /// Defaults to [`LimitConditions::ToGenesis`].
    pub limit_conditions: LimitConditions<H>,
    /// Should the query return any entries that are
    /// cached at the agent activity to save network hops.
    pub include_cached_entries: bool,
}

/// Specify when to stop walking down the chain from the `chain_top` action.
///
/// All variants walk down the chain from the `chain_top` action,
/// following the linear chain path and excluding any forked actions.
#[derive(Serialize, Deserialize, Debug, Eq, Clone, Default)]
pub enum LimitConditions<H: Eq + Ord + std::hash::Hash = ActionHash> {
    /// Include all actions to the end of the chain.
    #[default]
    ToGenesis,
    /// Include exactly the specified number of actions in the chain, or all actions in the chain, whichever is fewer.
    ///
    /// A value of `0` is considered invalid and will cause an error.
    Take(u32),
    /// Include all actions in the chain with timestamps greater than or equal to the given timestamp.
    ///
    /// To receive a success response, the query must retrieve an action with a timestamp *less* than the given timestamp,
    /// *or* retrieve all actions until genesis. Without this we would not know if there are additional
    /// actions within the timestamp, and so the response would not be deterministic.
    ///
    /// A timestamp value that is greater than the `chain_top` timestamp is considered invalid
    /// and will return `MustGetAgentActivityResponse::UntilTimestampGreaterThanChainHead`.
    UntilTimestamp(Timestamp),
    /// Include all actions in the chain down to and including the action matching the given hash.
    ///
    /// To receive a success response, the query must retrieve an action matching the given hash.
    ///
    /// A hash value that maps to an action with a sequence number greater than the
    /// `chain_top` sequence number is considered invalid
    /// and will return `MustGetAgentActivityResponse::UntilHashAfterChainHead`.
    UntilHash(H),
}

/// Create a deterministic hash to compare [LimitConditions].
impl<H: Eq + Ord + std::hash::Hash> core::hash::Hash for LimitConditions<H> {
    fn hash<HH: std::hash::Hasher>(&self, state: &mut HH) {
        core::mem::discriminant(self).hash(state);
        match self {
            LimitConditions::ToGenesis => (),
            LimitConditions::Take(t) => t.hash(state),
            LimitConditions::UntilTimestamp(ts) => ts.hash(state),
            LimitConditions::UntilHash(u) => u.hash(state),
        }
    }
}

/// Implement a deterministic partial eq to compare [LimitConditions].
impl<H: Eq + Ord + std::hash::Hash> core::cmp::PartialEq for LimitConditions<H> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Take(l0), Self::Take(r0)) => l0 == r0,
            (Self::UntilTimestamp(l0), Self::UntilTimestamp(r0)) => l0 == r0,
            (Self::UntilHash(l0), Self::UntilHash(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

/// Input to the `must_get_agent_activity` call.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MustGetAgentActivityInput {
    /// The author of the chain that you are requesting
    /// activity from.
    pub author: AgentPubKey,
    /// The filter on the chain's activity.
    pub chain_filter: ChainFilter,
}

impl<H: Eq + Ord + std::hash::Hash> ChainFilter<H> {
    /// Create a new filter using this [`ActionHash`] as
    /// the starting position and walking the chain
    /// towards the genesis [`Action`](crate::action::Action).
    pub fn new(chain_top: H) -> Self {
        Self {
            chain_top,
            limit_conditions: Default::default(),
            include_cached_entries: false,
        }
    }

    /// Create a filter with a take limit.
    pub fn take(chain_top: H, n: u32) -> Self {
        Self {
            chain_top,
            limit_conditions: LimitConditions::Take(n),
            include_cached_entries: false,
        }
    }

    /// Set this filter to include any cached entries
    /// at the agent activity authority.
    pub fn include_cached_entries(mut self) -> Self {
        self.include_cached_entries = true;
        self
    }

    /// Create a filter with an until-timestamp limit.
    pub fn until_timestamp(chain_top: H, timestamp: Timestamp) -> Self {
        Self {
            chain_top,
            limit_conditions: LimitConditions::UntilTimestamp(timestamp),
            include_cached_entries: false,
        }
    }

    /// Create a filter with an until-hash limit.
    pub fn until_hash(chain_top: H, action_hash: H) -> Self {
        Self {
            chain_top,
            limit_conditions: LimitConditions::UntilHash(action_hash),
            include_cached_entries: false,
        }
    }

    /// Get the until hash if there is one.
    pub fn get_until_hash(&self) -> Option<&H> {
        match &self.limit_conditions {
            LimitConditions::UntilHash(u) => Some(u),
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_take(&self) -> Option<u32> {
        match self.limit_conditions {
            LimitConditions::Take(n) => Some(n),
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_until_timestamp(&self) -> Option<Timestamp> {
        match self.limit_conditions {
            LimitConditions::UntilTimestamp(ts) => Some(ts),
            _ => None,
        }
    }
}

/// Input to close a chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct CloseChainInput {
    /// The target identifier for the chain that will be migrated to.
    pub new_target: Option<MigrationTarget>,
}

/// Input to open a chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct OpenChainInput {
    /// The identifier for the chain that was migrated from.
    pub prev_target: MigrationTarget,

    /// Hash of the corresponding CloseChain action
    pub close_hash: ActionHash,
}
