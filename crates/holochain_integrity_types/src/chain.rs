//! # Source Chain Filtering
//! Types for filtering the source chain.

use std::collections::HashSet;

use crate::MigrationTarget;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;

#[cfg(test)]
mod test;

#[derive(Serialize, Deserialize, SerializedBytes, Debug, PartialEq, Eq, Hash, Clone)]
/// Filter source chain items.
/// Starting from some chain position given as an [`ActionHash`]
/// the chain is walked backwards to genesis.
/// The filter can stop early by specifying stop conditions:
/// A maximum number of items is reached, a given [`ActionHash`] is found, or
/// a given timestamp has been passed.
/// Multiple `[`ActionHash`] can be provided. The filter will stop at and take the first one found.
/// When providing a Timestamp, the filter will stop at and take the last item that has a
/// Timestamp younger than the provided one. In the edge case of multiple actions having the same
/// Timestamp as the stop condition, only one of the actions will be included.
pub struct ChainFilter<H: Eq + Ord + std::hash::Hash = ActionHash> {
    /// The starting position of the filter.
    pub chain_top: H,
    /// The stop conditions used by this filter.
    /// Defaults to [`StopConditions::ToGenesis`].
    pub stop_conditions: StopConditions<H>,
    /// Should the query return any entries that are
    /// cached at the agent activity to save network hops.
    pub include_cached_entries: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
/// Specify when to stop walking down the chain.
pub enum StopConditions<H: Eq + Ord + std::hash::Hash = ActionHash> {
    /// Allow all up to genesis.
    ToGenesis,
    /// Take this many items (inclusive of the starting position).
    Take(u32),
    /// Continue until some timestamp is passed.
    UntilTimestamp(Timestamp),
    /// Continue until one of these hashes is found.
    UntilHash(HashSet<H>),
    /// Combination of Take, UntilTimestamp and UntilHash.
    /// Whichever is the smaller set.
    Multiple(u32, HashSet<H>, Timestamp),
}

/// Create a deterministic hash to compare StopConditions.
impl<H: Eq + Ord + std::hash::Hash> core::hash::Hash for StopConditions<H> {
    fn hash<HH: std::hash::Hasher>(&self, state: &mut HH) {
        core::mem::discriminant(self).hash(state);
        match self {
            StopConditions::ToGenesis => (),
            StopConditions::Take(t) => t.hash(state),
            StopConditions::UntilTimestamp(ts) => ts.hash(state),
            StopConditions::UntilHash(u) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
            }
            StopConditions::Multiple(n, u, t) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
                n.hash(state);
                t.hash(state);
            }
        }
    }
}

/// Implement a deterministic partial eq to compare StopConditions.
impl<H: Eq + Ord + std::hash::Hash> core::cmp::PartialEq for StopConditions<H> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Take(l0), Self::Take(r0)) => l0 == r0,
            (Self::UntilTimestamp(l0), Self::UntilTimestamp(r0)) => l0 == r0,
            (Self::UntilHash(a), Self::UntilHash(b)) => {
                let mut a: Vec<_> = a.iter().collect();
                let mut b: Vec<_> = b.iter().collect();
                a.sort_unstable();
                b.sort_unstable();
                a == b
            }
            (Self::Multiple(l0, a, t0), Self::Multiple(r0, b, t1)) => {
                let mut a: Vec<_> = a.iter().collect();
                let mut b: Vec<_> = b.iter().collect();
                a.sort_unstable();
                b.sort_unstable();
                l0 == r0 && a == b && t0 == t1
            }
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
/// Input of the `must_get_agent_activity` call.
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
            stop_conditions: Default::default(),
            include_cached_entries: false,
        }
    }

    /// Take up to `n` actions including the starting position.
    /// This may return less then `n` actions.
    pub fn take(mut self, n: u32) -> Self {
        self.stop_conditions = match self.stop_conditions {
            StopConditions::ToGenesis => StopConditions::Take(n),
            StopConditions::Take(old_n) => StopConditions::Take(old_n.min(n)),
            StopConditions::UntilTimestamp(t) => StopConditions::Multiple(n, HashSet::new(), t),
            StopConditions::UntilHash(u) => StopConditions::Multiple(n, u, Timestamp(0)),
            StopConditions::Multiple(old_n, u, t) => StopConditions::Multiple(old_n.min(n), u, t),
        };
        self
    }

    /// Set this filter to include any cached entries
    /// at the agent activity authority.
    pub fn include_cached_entries(mut self) -> Self {
        self.include_cached_entries = true;
        self
    }

    /// Take all actions until this timestamp is passed.
    pub fn until_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.stop_conditions = match self.stop_conditions {
            StopConditions::ToGenesis => StopConditions::UntilTimestamp(timestamp),
            StopConditions::Take(n) => StopConditions::Multiple(n, HashSet::new(), timestamp),
            StopConditions::UntilTimestamp(old_ts) => {
                StopConditions::UntilTimestamp(timestamp.max(old_ts))
            }
            StopConditions::UntilHash(u) => StopConditions::Multiple(u32::MAX, u, timestamp),
            StopConditions::Multiple(n, u, _) => StopConditions::Multiple(n, u, timestamp),
        };
        self
    }

    /// Take all actions until this action hash is found.
    /// Note that all actions specified as `until_hash` hashes must be
    /// found so this filter can produce deterministic results.
    /// It is invalid to specify an until hash that is on a different
    /// fork then the starting position.
    pub fn until_hash(mut self, action_hash: H) -> Self {
        self.stop_conditions = match self.stop_conditions {
            StopConditions::ToGenesis => {
                StopConditions::UntilHash(Some(action_hash).into_iter().collect())
            }
            StopConditions::Take(n) => {
                StopConditions::Multiple(n, Some(action_hash).into_iter().collect(), Timestamp(0))
            }
            StopConditions::UntilTimestamp(t) => {
                StopConditions::Multiple(u32::MAX, Some(action_hash).into_iter().collect(), t)
            }
            StopConditions::UntilHash(mut u) => {
                u.insert(action_hash);
                StopConditions::UntilHash(u)
            }
            StopConditions::Multiple(n, mut u, t) => {
                u.insert(action_hash);
                StopConditions::Multiple(n, u, t)
            }
        };
        self
    }

    /// Get the until hashes if there are any.
    pub fn get_until_hash(&self) -> Option<&HashSet<H>> {
        match &self.stop_conditions {
            StopConditions::UntilHash(u) => Some(u),
            StopConditions::Multiple(_, u, _) => Some(u),
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_take(&self) -> Option<u32> {
        match &self.stop_conditions {
            StopConditions::Take(n) => Some(*n),
            StopConditions::Multiple(n, _, _) => {
                if *n != u32::MAX {
                    Some(*n)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_until_timestamp(&self) -> Option<Timestamp> {
        match &self.stop_conditions {
            StopConditions::UntilTimestamp(ts) => Some(*ts),
            StopConditions::Multiple(_, _, ts) => {
                if ts.0 > 0 {
                    Some(*ts)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl<H: Eq + Ord + std::hash::Hash> Default for StopConditions<H> {
    fn default() -> Self {
        Self::ToGenesis
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
