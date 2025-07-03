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
/// The filter can stop early by specifying the number of
/// chain items to take and / or an [`ActionHash`] to consume until.
pub struct ChainFilter<H: Eq + Ord + std::hash::Hash = ActionHash> {
    /// The starting position of the filter.
    pub chain_top: H,
    /// The filters that have been applied.
    /// Defaults to [`ChainFilters::ToGenesis`].
    pub filters: ChainFilters<H>,
    /// Should the query return any entries that are
    /// cached at the agent activity to save network hops.
    pub include_cached_entries: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
/// Specify which [`Action`](crate::action::Action)s to allow through
/// this filter.
pub enum ChainFilters<H: Eq + Ord + std::hash::Hash = ActionHash> {
    /// Allow all up to genesis.
    ToGenesis,
    /// Take this many (inclusive of the starting position).
    Take(u32),
    /// Continue until timestamp is reached
    UntilTimestamp(Timestamp),
    /// Continue until one of these hashes is found.
    UntilHash(HashSet<H>),
    /// Combination of take, until_hash and until_timestamp.
    /// Whichever is the smaller set.
    Multiple(u32, HashSet<H>, Timestamp),
}

/// Create a deterministic hash to compare filters.
impl<H: Eq + Ord + std::hash::Hash> core::hash::Hash for ChainFilters<H> {
    fn hash<HH: std::hash::Hasher>(&self, state: &mut HH) {
        core::mem::discriminant(self).hash(state);
        match self {
            ChainFilters::ToGenesis => (),
            ChainFilters::Take(t) => t.hash(state),
            ChainFilters::UntilTimestamp(ts) => ts.hash(state),
            ChainFilters::UntilHash(u) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
            }
            ChainFilters::Multiple(n, u, t) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
                n.hash(state);
                t.hash(state);
            }
        }
    }
}

/// Implement a deterministic partial eq to compare ChainFilters.
impl<H: Eq + Ord + std::hash::Hash> core::cmp::PartialEq for ChainFilters<H> {
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
    /// The filter on the chains activity.
    pub chain_filter: ChainFilter,
}

impl<H: Eq + Ord + std::hash::Hash> ChainFilter<H> {
    /// Create a new filter using this [`ActionHash`] as
    /// the starting position and walking the chain
    /// towards the genesis [`Action`](crate::action::Action).
    pub fn new(chain_top: H) -> Self {
        Self {
            chain_top,
            filters: Default::default(),
            include_cached_entries: false,
        }
    }

    /// Take up to `n` actions including the starting position.
    /// This may return less then `n` actions.
    pub fn take(mut self, n: u32) -> Self {
        self.filters = match self.filters {
            ChainFilters::ToGenesis => ChainFilters::Take(n),
            ChainFilters::Take(old_n) => ChainFilters::Take(old_n.min(n)),
            ChainFilters::UntilTimestamp(t) => ChainFilters::Multiple(n, HashSet::new(), t),
            ChainFilters::UntilHash(u) => ChainFilters::Multiple(n, u, Timestamp(0)),
            ChainFilters::Multiple(old_n, u, t) => ChainFilters::Multiple(old_n.min(n), u, t),
        };
        self
    }

    /// Set this filter to include any cached entries
    /// at the agent activity authority.
    pub fn include_cached_entries(mut self) -> Self {
        self.include_cached_entries = true;
        self
    }

    /// Take all actions until this timestamp is reached
    pub fn until_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.filters = match self.filters {
            ChainFilters::ToGenesis => ChainFilters::UntilTimestamp(timestamp),
            ChainFilters::Take(n) => ChainFilters::Multiple(n, HashSet::new(), timestamp),
            ChainFilters::UntilTimestamp(_) => ChainFilters::UntilTimestamp(timestamp),
            ChainFilters::UntilHash(u) => ChainFilters::Multiple(u32::MAX, u, timestamp),
            ChainFilters::Multiple(n, u, _) => ChainFilters::Multiple(n, u, timestamp),
        };
        self
    }

    /// Take all actions until this action hash is found.
    /// Note that all actions specified as `until_hash` hashes must be
    /// found so this filter can produce deterministic results.
    /// It is invalid to specify an until hash that is on a different
    /// fork then the starting position.
    pub fn until_hash(mut self, action_hash: H) -> Self {
        self.filters = match self.filters {
            ChainFilters::ToGenesis => {
                ChainFilters::UntilHash(Some(action_hash).into_iter().collect())
            }
            ChainFilters::Take(n) => {
                ChainFilters::Multiple(n, Some(action_hash).into_iter().collect(), Timestamp(0))
            }
            ChainFilters::UntilTimestamp(t) => {
                ChainFilters::Multiple(u32::MAX, Some(action_hash).into_iter().collect(), t)
            }
            ChainFilters::UntilHash(mut u) => {
                u.insert(action_hash);
                ChainFilters::UntilHash(u)
            }
            ChainFilters::Multiple(n, mut u, t) => {
                u.insert(action_hash);
                ChainFilters::Multiple(n, u, t)
            }
        };
        self
    }

    /// Get the until hashes if there are any.
    pub fn get_until_hash(&self) -> Option<&HashSet<H>> {
        match &self.filters {
            ChainFilters::UntilHash(u) => Some(u),
            ChainFilters::Multiple(_, u, _) => Some(u),
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_take(&self) -> Option<u32> {
        match &self.filters {
            ChainFilters::Take(n) => Some(*n),
            ChainFilters::Multiple(n, _, _) => {
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
        match &self.filters {
            ChainFilters::UntilTimestamp(ts) => Some(*ts),
            ChainFilters::Multiple(_, _, ts) => {
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

impl<H: Eq + Ord + std::hash::Hash> Default for ChainFilters<H> {
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
