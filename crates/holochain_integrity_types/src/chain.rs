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
/// The filter can stop early by specifying limit conditions:
/// A maximum number of items is reached, a given [`ActionHash`] is found, or
/// a given timestamp has been passed.
/// Multiple [`ActionHash`]es can be provided. The filter will stop at and take the first one found.
/// When providing a Timestamp, the filter will stop at and take the oldest action that has a
/// Timestamp newer than the provided one. In the edge case of multiple actions having the same
/// Timestamp as the limit condition, the filter will stop at the action with the lowest sequence.
/// Multiple limit conditions can be set. Whichever is the smaller set will be kept.
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

#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
/// Specify when to stop walking down the chain.
pub enum LimitConditions<H: Eq + Ord + std::hash::Hash = ActionHash> {
    /// Allow all up to genesis.
    ToGenesis,
    /// Take this many actions (inclusive of the starting position).
    Take(u32),
    /// Take all actions since the given timestamp.
    UntilTimestamp(Timestamp),
    /// Continue until one of these hashes is found.
    UntilHash(HashSet<H>),
    /// Combination of Take, UntilTimestamp and UntilHash.
    Multiple(Option<u32>, HashSet<H>, Option<Timestamp>),
}

/// Create a deterministic hash to compare [LimitConditions].
impl<H: Eq + Ord + std::hash::Hash> core::hash::Hash for LimitConditions<H> {
    fn hash<HH: std::hash::Hasher>(&self, state: &mut HH) {
        core::mem::discriminant(self).hash(state);
        match self {
            LimitConditions::ToGenesis => (),
            LimitConditions::Take(t) => t.hash(state),
            LimitConditions::UntilTimestamp(ts) => ts.hash(state),
            LimitConditions::UntilHash(u) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
            }
            LimitConditions::Multiple(n, u, t) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
                n.hash(state);
                t.hash(state);
            }
        }
    }
}

/// Implement a deterministic partial eq to compare [LimitConditions].
impl<H: Eq + Ord + std::hash::Hash> core::cmp::PartialEq for LimitConditions<H> {
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
/// Input to the `must_get_agent_activity` call.
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

    /// Take up to `n` actions including the starting position.
    /// This may return less than `n` actions.
    /// If an `n` was already set, the smallest of the two is kept.
    pub fn take(mut self, n: u32) -> Self {
        self.limit_conditions = match self.limit_conditions {
            LimitConditions::ToGenesis => LimitConditions::Take(n),
            LimitConditions::Take(old_n) => LimitConditions::Take(old_n.min(n)),
            LimitConditions::UntilTimestamp(t) => {
                LimitConditions::Multiple(Some(n), HashSet::new(), Some(t))
            }
            LimitConditions::UntilHash(u) => LimitConditions::Multiple(Some(n), u, None),
            LimitConditions::Multiple(old_n, u, t) => {
                LimitConditions::Multiple(Some(old_n.unwrap_or(u32::MAX).min(n)), u, t)
            }
        };
        self
    }

    /// Set this filter to include any cached entries
    /// at the agent activity authority.
    pub fn include_cached_entries(mut self) -> Self {
        self.include_cached_entries = true;
        self
    }

    /// Take all actions since the given timestamp.
    ///
    /// If a timestamp was already set, the biggest of the two is kept.
    pub fn until_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.limit_conditions = match self.limit_conditions {
            LimitConditions::ToGenesis => LimitConditions::UntilTimestamp(timestamp),
            LimitConditions::Take(n) => {
                LimitConditions::Multiple(Some(n), HashSet::new(), Some(timestamp))
            }
            LimitConditions::UntilTimestamp(old_ts) => {
                LimitConditions::UntilTimestamp(timestamp.max(old_ts))
            }
            LimitConditions::UntilHash(u) => LimitConditions::Multiple(None, u, Some(timestamp)),
            LimitConditions::Multiple(n, u, old_ts) => {
                LimitConditions::Multiple(n, u, Some(old_ts.unwrap_or(Timestamp(0)).max(timestamp)))
            }
        };
        self
    }

    /// Take all actions until this action hash is found.
    /// Note that all actions specified as `until_hash` hashes must be
    /// found so this filter can produce deterministic results.
    /// It is invalid to specify an until hash that is on a different
    /// fork then the starting position.
    pub fn until_hash(mut self, action_hash: H) -> Self {
        self.limit_conditions = match self.limit_conditions {
            LimitConditions::ToGenesis => {
                LimitConditions::UntilHash(Some(action_hash).into_iter().collect())
            }
            LimitConditions::Take(n) => {
                LimitConditions::Multiple(Some(n), Some(action_hash).into_iter().collect(), None)
            }
            LimitConditions::UntilTimestamp(t) => {
                LimitConditions::Multiple(None, Some(action_hash).into_iter().collect(), Some(t))
            }
            LimitConditions::UntilHash(mut u) => {
                u.insert(action_hash);
                LimitConditions::UntilHash(u)
            }
            LimitConditions::Multiple(n, mut u, t) => {
                u.insert(action_hash);
                LimitConditions::Multiple(n, u, t)
            }
        };
        self
    }

    /// Get the until hashes if there are any.
    pub fn get_until_hash(&self) -> Option<&HashSet<H>> {
        match &self.limit_conditions {
            LimitConditions::UntilHash(u) => Some(u),
            LimitConditions::Multiple(_, u, _) => Some(u),
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_take(&self) -> Option<u32> {
        match self.limit_conditions {
            LimitConditions::Take(n) => Some(n),
            LimitConditions::Multiple(n, _, _) => n,
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_until_timestamp(&self) -> Option<Timestamp> {
        match self.limit_conditions {
            LimitConditions::UntilTimestamp(ts) => Some(ts),
            LimitConditions::Multiple(_, _, ts) => ts,
            _ => None,
        }
    }
}

impl<H: Eq + Ord + std::hash::Hash> Default for LimitConditions<H> {
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
