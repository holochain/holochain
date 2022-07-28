//! # Source Chain Filtering
//! Types for filtering the source chain.

use std::collections::HashSet;

use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

#[cfg(test)]
mod test;

#[derive(Serialize, Deserialize, SerializedBytes, Debug, PartialEq, Eq, Hash, Clone)]
/// Filter source chain items.
/// Starting from some chain position given as an [`ActionHash`]
/// the chain is walked backwards to genesis.
/// The filter can stop early by specifying the number of
/// chain items to take and / or an [`ActionHash`] to consume until.
pub struct ChainFilter {
    /// The starting position of the filter.
    pub chain_top: ActionHash,
    /// The filters that have been applied.
    /// Defaults to [`ChainFilters::ToGenesis`].
    pub filters: ChainFilters,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
/// Specify which [`Action`](crate::action::Action)s to allow through
/// this filter.
pub enum ChainFilters {
    /// Allow all up to genesis.
    ToGenesis,
    /// Take this many (inclusive of the starting position).
    Take(u32),
    /// Continue until one of these hashes is found.
    Until(HashSet<ActionHash>),
    /// Combination of both take and until.
    /// Whichever is the smaller set.
    Both(u32, HashSet<ActionHash>),
}

/// Create a deterministic hash to compare filters.
impl core::hash::Hash for ChainFilters {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            ChainFilters::ToGenesis => (),
            ChainFilters::Take(t) => t.hash(state),
            ChainFilters::Until(u) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
            }
            ChainFilters::Both(t, u) => {
                let mut u: Vec<_> = u.iter().collect();
                u.sort_unstable();
                u.hash(state);
                t.hash(state);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
/// Input to the `must_get_agent_activity` call.
pub struct MustGetAgentActivityInput {
    /// The author of the chain that you are requesting
    /// activity from.
    pub author: AgentPubKey,
    /// The filter on the chains activity.
    pub chain_filter: ChainFilter,
}

impl ChainFilter {
    /// Create a new filter using this [`ActionHash`] as
    /// the starting position and walking the chain
    /// towards the genesis [`Action`](crate::action::Action).
    pub fn new(chain_top: ActionHash) -> Self {
        Self {
            chain_top,
            filters: Default::default(),
        }
    }

    /// Take up to `n` actions including the starting position.
    /// This may return less then `n` actions.
    pub fn take(mut self, n: u32) -> Self {
        self.filters = match self.filters {
            ChainFilters::ToGenesis => ChainFilters::Take(n),
            ChainFilters::Take(old_n) => ChainFilters::Take(old_n.min(n)),
            ChainFilters::Until(u) => ChainFilters::Both(n, u),
            ChainFilters::Both(old_n, u) => ChainFilters::Both(old_n.min(n), u),
        };
        self
    }

    /// Take all actions until this action hash is found.
    /// Note that all actions specified as `until` hashes must be
    /// found so this filter can produce deterministic results.
    /// It is invalid to specify an until hash that is on a different
    /// fork then the starting position.
    pub fn until(mut self, action_hash: ActionHash) -> Self {
        self.filters = match self.filters {
            ChainFilters::ToGenesis => ChainFilters::Until(Some(action_hash).into_iter().collect()),
            ChainFilters::Take(n) => ChainFilters::Both(n, Some(action_hash).into_iter().collect()),
            ChainFilters::Until(mut u) => {
                u.insert(action_hash);
                ChainFilters::Until(u)
            }
            ChainFilters::Both(n, mut u) => {
                u.insert(action_hash);
                ChainFilters::Both(n, u)
            }
        };
        self
    }

    /// Get the until hashes if there are any.
    pub fn get_until(&self) -> Option<&HashSet<ActionHash>> {
        match &self.filters {
            ChainFilters::Until(u) => Some(u),
            ChainFilters::Both(_, u) => Some(u),
            _ => None,
        }
    }

    /// Get the take number if there is one.
    pub fn get_take(&self) -> Option<u32> {
        match &self.filters {
            ChainFilters::Take(s) => Some(*s),
            ChainFilters::Both(s, _) => Some(*s),
            _ => None,
        }
    }
}

impl Default for ChainFilters {
    fn default() -> Self {
        Self::ToGenesis
    }
}
