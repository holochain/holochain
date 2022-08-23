//! # Source Chain Filtering
//! Types for filtering the source chain.

use std::collections::HashSet;

use holo_hash::{ActionHash, HasHash};
use holochain_serialized_bytes::prelude::*;

use crate::{ActionHashed, SignedActionHashed};

#[cfg(test)]
mod test;

#[derive(Serialize, Deserialize, SerializedBytes, Debug, PartialEq, Eq, Clone)]
/// Filter source chain items.
/// Starting from some chain position given as an [`ActionHash`]
/// the chain is walked backwards to genesis.
/// The filter can stop early by specifying the number of
/// chain items to take and / or an [`ActionHash`] to consume until.
pub struct ChainFilter<H: Eq + std::hash::Hash = ActionHash> {
    /// The starting position of the filter.
    pub chain_top: H,
    /// The filters that have been applied.
    /// Defaults to [`ChainFilters::ToGenesis`].
    pub filters: ChainFilters<H>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
/// Specify which [`Action`](crate::action::Action)s to allow through
/// this filter.
pub enum ChainFilters<H: Eq + std::hash::Hash = ActionHash> {
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

impl<H: Eq + std::hash::Hash> ChainFilter<H> {
    /// Create a new filter using this [`ActionHash`] as
    /// the starting position and walking the chain
    /// towards the genesis [`Action`](crate::action::Action).
    pub fn new(chain_top: H) -> Self {
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

impl<H: Eq + std::hash::Hash> Default for ChainFilters<H> {
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
