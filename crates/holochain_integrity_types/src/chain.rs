//! # Source Chain Filtering
//! Types for filtering the source chain.

use std::collections::HashSet;

use holo_hash::AgentPubKey;
use holo_hash::{ActionHash, HasHash};
use holochain_serialized_bytes::prelude::*;

use crate::{ActionHashed, SignedActionHashed};

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
    /// Continue until one of these hashes is found.
    Until(HashSet<H>),
    /// Combination of both take and until.
    /// Whichever is the smaller set.
    Both(u32, HashSet<H>),
}

/// Create a deterministic hash to compare filters.
impl<H: Eq + Ord + std::hash::Hash> core::hash::Hash for ChainFilters<H> {
    fn hash<HH: std::hash::Hasher>(&self, state: &mut HH) {
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

/// Implement a deterministic partial eq to compare ChainFilters.
impl<H: Eq + Ord + std::hash::Hash> core::cmp::PartialEq for ChainFilters<H> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Take(l0), Self::Take(r0)) => l0 == r0,
            (Self::Until(a), Self::Until(b)) => {
                let mut a: Vec<_> = a.iter().collect();
                let mut b: Vec<_> = b.iter().collect();
                a.sort_unstable();
                b.sort_unstable();
                a == b
            }
            (Self::Both(l0, a), Self::Both(r0, b)) => {
                let mut a: Vec<_> = a.iter().collect();
                let mut b: Vec<_> = b.iter().collect();
                a.sort_unstable();
                b.sort_unstable();
                l0 == r0 && a == b
            }
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
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
            ChainFilters::Until(u) => ChainFilters::Both(n, u),
            ChainFilters::Both(old_n, u) => ChainFilters::Both(old_n.min(n), u),
        };
        self
    }

    /// Set this filter to include any cached entries
    /// at the agent activity authority.
    pub fn include_cached_entries(mut self) -> Self {
        self.include_cached_entries = true;
        self
    }

    /// Take all actions until this action hash is found.
    /// Note that all actions specified as `until` hashes must be
    /// found so this filter can produce deterministic results.
    /// It is invalid to specify an until hash that is on a different
    /// fork then the starting position.
    pub fn until(mut self, action_hash: H) -> Self {
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
    pub fn get_until(&self) -> Option<&HashSet<H>> {
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

impl<H: Eq + Ord + std::hash::Hash> Default for ChainFilters<H> {
    fn default() -> Self {
        Self::ToGenesis
    }
}

/// Abstraction over an item in a chain.
// Alternate implementations are only used for testing, so this should not
// add a large monomorphization overhead
pub trait ChainItem: Clone + PartialEq + Eq + std::fmt::Debug {
    /// The type used to represent a hash of this item
    type Hash: Clone
        + PartialEq
        + Eq
        + Ord
        + std::hash::Hash
        + std::fmt::Debug
        + Send
        + Sync
        + Into<ActionHash>;

    /// The sequence in the chain
    fn seq(&self) -> u32;

    /// The hash of this item
    fn get_hash(&self) -> &Self::Hash;

    /// The hash of the previous item
    fn prev_hash(&self) -> Option<&Self::Hash>;
}

impl ChainItem for ActionHashed {
    type Hash = ActionHash;

    fn seq(&self) -> u32 {
        self.action_seq()
    }

    fn get_hash(&self) -> &Self::Hash {
        self.as_hash()
    }

    fn prev_hash(&self) -> Option<&Self::Hash> {
        self.prev_action()
    }
}

impl ChainItem for SignedActionHashed {
    type Hash = ActionHash;

    fn seq(&self) -> u32 {
        self.hashed.seq()
    }

    fn get_hash(&self) -> &Self::Hash {
        self.hashed.get_hash()
    }

    fn prev_hash(&self) -> Option<&Self::Hash> {
        self.hashed.prev_hash()
    }
}
