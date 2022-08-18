//! Types related to an agents for chain activity
use std::iter::Peekable;
use std::ops::RangeInclusive;

use crate::activity::AgentActivityResponse;
use crate::activity::ChainItems;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::HasHash;
use holochain_zome_types::prelude::ChainStatus;
use holochain_zome_types::ActionHashed;
use holochain_zome_types::ChainFilter;
use holochain_zome_types::ChainFilters;
use holochain_zome_types::RegisterAgentActivity;
use holochain_zome_types::SignedActionHashed;

#[cfg(all(test, feature = "test_utils"))]
pub mod test;

/// Helpers for constructing AgentActivity
pub trait AgentActivityExt {
    /// Create an empty chain status
    fn empty<T>(agent: &AgentPubKey) -> AgentActivityResponse<T> {
        AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
        }
    }
}

impl AgentActivityExt for AgentActivityResponse {}

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

#[must_use = "Iterator doesn't do anything unless consumed."]
#[derive(Debug)]
/// Iterate over a source chain and apply the [`ChainFilter`] to each element.
/// This iterator will:
/// - Ignore any ops that are not a direct ancestor to the starting position.
/// - Stop at the first gap in the chain.
/// - Take no **more** then the [`take`]. It may return less.
/// - Stop at (including) the [`ActionHash`](holo_hash::ActionHash) in [`until`]. But not if this hash is not in the chain.
///
/// [`take`]: ChainFilter::take
/// [`until`]: ChainFilter::until
pub struct ChainFilterIter<I: AsRef<A>, A: ChainItem = SignedActionHashed> {
    filter: ChainFilter<A::Hash>,
    iter: Peekable<std::vec::IntoIter<I>>,
    end: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A [`ChainFilter`] with the action sequences for the
/// starting position and any `until` hashes.
pub struct ChainFilterRange {
    /// The filter for for this chain.
    filter: ChainFilter,
    /// The start of this range is the end of
    /// the filter iterator.
    /// The end of this range is the sequence of
    /// the starting position hash.
    range: RangeInclusive<u32>,
    /// The start of the range's type.
    chain_bottom_type: ChainBottomType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The type of chain item that forms the bottom of the chain.
enum ChainBottomType {
    /// The bottom of the chain is genesis.
    Genesis,
    /// The bottom of the chain is the action where `take`
    /// has reached zero.
    Take,
    /// The bottom of the chain is the action where an
    /// `until` hash was found.
    Until,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Outcome of trying to find the action sequences in a filter.
pub enum Sequences {
    /// Found all action sequences
    Found(ChainFilterRange),
    /// The chain top action was not found.
    ChainTopNotFound(ActionHash),
    /// The filter produces an empty range.
    EmptyRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Response to a `must_get_agent_activity` call.
pub enum MustGetAgentActivityResponse {
    /// The activity was found.
    Activity(Vec<RegisterAgentActivity>),
    /// The requested chain range was incomplete.
    IncompleteChain,
    /// The requested chain top was not found in the chain.
    ChainTopNotFound(ActionHash),
    /// The filter produces an empty range.
    EmptyRange,
}

impl<I: AsRef<A>, A: ChainItem> ChainFilterIter<I, A> {
    /// Create an iterator that filters an iterator of actions
    /// with a [`ChainFilter`].
    ///
    /// # Constraints
    /// - If the iterator does not contain the filter's chain_top
    /// then this will be an empty iterator.
    pub fn new(filter: ChainFilter<A::Hash>, mut chain: Vec<I>) -> Self {
        // Sort by descending.
        chain.sort_unstable_by_key(|a| u32::MAX - a.as_ref().seq());
        // Create a peekable iterator.
        let mut iter = chain.into_iter().peekable();

        // Discard any ops that are not the chain_top.
        let i = iter.by_ref();
        while let Some(op) = i.peek() {
            if *op.as_ref().get_hash() == filter.chain_top {
                break;
            }
            i.next();
        }

        Self {
            filter,
            iter,
            end: false,
        }
    }
}

impl<I: AsRef<A>, A: ChainItem> Iterator for ChainFilterIter<I, A> {
    type Item = I;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end {
            return None;
        }

        let op = self.iter.next()?;
        let op = loop {
            let parent = self.iter.peek();

            // Check the next sequence number
            match parent {
                Some(parent) => {
                    let child_seq = op.as_ref().seq();
                    let parent_seq = parent.as_ref().seq();
                    match (child_seq.cmp(&parent_seq), op.as_ref().prev_hash()) {
                        (std::cmp::Ordering::Less, _) => {
                            // The chain is out of order so we must end here.
                            self.end = true;
                            break op;
                        }
                        (std::cmp::Ordering::Equal, _) => {
                            // There is a fork in the chain.
                            // Discard this parent.
                            self.iter.next();
                            // Try the next parent.
                            continue;
                        }
                        (std::cmp::Ordering::Greater, None) => {
                            // The chain is correct however there is no previous action for this child.
                            // The child can't be the first chain item and doesn't have a parent like:
                            // `child != 0 && child -> ()`.
                            // All we can do is end the iterator.
                            // I don't think this state is actually reachable
                            // because the only header that can have no previous action is the `Dna` and
                            // it is always zero.
                            return None;
                        }
                        (std::cmp::Ordering::Greater, _)
                            if parent_seq.checked_add(1)? != child_seq =>
                        {
                            // There is a gap in the chain so we must end here.
                            self.end = true;
                            break op;
                        }
                        (std::cmp::Ordering::Greater, Some(prev_hash))
                            if prev_hash != parent.as_ref().get_hash() =>
                        {
                            // Not the parent of this child.
                            // Discard this parent.
                            self.iter.next();
                            // Try the next parent.
                            continue;
                        }
                        (std::cmp::Ordering::Greater, Some(_)) => {
                            // Correct parent found.
                            break op;
                        }
                    }
                }
                None => break op,
            }
        };

        match &mut self.filter.filters {
            // Check if there is any left to take.
            ChainFilters::Take(n) => *n = n.checked_sub(1)?,
            // Check if the `until` hash has been found.
            ChainFilters::Until(until_hashes) => {
                if until_hashes.contains(op.as_ref().get_hash()) {
                    // If it has, include it and return on the next call to `next`.
                    self.end = true;
                }
            }
            // Just keep going till genesis.
            ChainFilters::ToGenesis => (),
            // Both filters are active. Return on the first to be hit.
            ChainFilters::Both(n, until_hashes) => {
                *n = n.checked_sub(1)?;

                if until_hashes.contains(op.as_ref().get_hash()) {
                    self.end = true;
                }
            }
        }
        Some(op)
    }
}

impl Sequences {
    /// Find the action sequences for all hashes in the filter.
    pub fn find_sequences<F, E>(filter: ChainFilter, mut get_seq: F) -> Result<Self, E>
    where
        F: FnMut(&ActionHash) -> Result<Option<u32>, E>,
    {
        // Get the top of the chain action sequence.
        // This is the highest sequence number and also the
        // start of the iterator.
        let chain_top = match get_seq(&filter.chain_top)? {
            Some(seq) => seq,
            None => return Ok(Self::ChainTopNotFound(filter.chain_top)),
        };

        // Track why the sequence start of the range was chosen.
        let mut chain_bottom_type = ChainBottomType::Genesis;

        // If their are any until hashes in the filter,
        // then find the highest sequence of the set
        // and find the distance from the position.
        let distance = match filter.get_until() {
            Some(until_hashes) => {
                // Find the highest sequence of the until hashes.
                let max = until_hashes
                    .iter()
                    .filter_map(|hash| {
                        match get_seq(hash) {
                            Ok(seq) => {
                                // Ignore any until hashes that could not be found.
                                let seq = seq?;
                                // Ignore any until hashes that are higher then a chain top.
                                (seq <= chain_top).then(|| Ok(seq))
                            }
                            Err(e) => Some(Err(e)),
                        }
                    })
                    .try_fold(0, |max, result| {
                        let seq = result?;
                        Ok(max.max(seq))
                    })?;

                if max != 0 {
                    // If the max is not genesis then there is an
                    // until hash that was found.
                    chain_bottom_type = ChainBottomType::Until;
                }

                // The distance from the chain top till highest until hash.
                // Note this cannot be an overflow due to the check above.
                chain_top - max
            }
            // If there is no until hashes then the distance is the chain top
            // till genesis (or just the chain top).
            None => chain_top,
        };

        // Check if there is a take filter and if that
        // will be reached before any until hashes or genesis.
        let start = match filter.get_take() {
            Some(take) => {
                // A take of zero will produce an empty range.
                if take == 0 {
                    return Ok(Self::EmptyRange);
                } else if take <= distance {
                    // The take will be reached before genesis or until hashes.
                    chain_bottom_type = ChainBottomType::Take;
                    // Add one to include the "position" in the number of
                    // "take". This matches the rust iterator "take".
                    chain_top.saturating_sub(take).saturating_add(1)
                } else {
                    // The range spans from the position for the distance
                    // that was determined earlier.
                    chain_top - distance
                }
            }
            // The range spans from the position for the distance
            // that was determined earlier.
            None => chain_top - distance,
        };
        Ok(Self::Found(ChainFilterRange {
            filter,
            range: start..=chain_top,
            chain_bottom_type,
        }))
    }
}

impl ChainFilterRange {
    /// Get the range of action sequences for this filter.
    pub fn range(&self) -> &RangeInclusive<u32> {
        &self.range
    }
    /// Filter the chain items then check the invariants hold.
    pub fn filter_then_check(
        self,
        chain: Vec<RegisterAgentActivity>,
    ) -> MustGetAgentActivityResponse {
        let until_hashes = self.filter.get_until().cloned();

        // Create the filter iterator and collect the filtered actions.
        let out: Vec<_> = ChainFilterIter::new(self.filter, chain).collect();

        // Check the invariants hold.
        match out.last().zip(out.first()) {
            // The actual results after the filter must match the range.
            Some((lowest, highest))
                if (lowest.action.action().action_seq()..=highest.action.action().action_seq())
                    == self.range =>
            {
                // If the range start was an until hash then the first action must
                // actually be an action from the until set.
                if let Some(hashes) = until_hashes {
                    if matches!(self.chain_bottom_type, ChainBottomType::Until)
                        && !hashes.contains(lowest.action.action_address())
                    {
                        return MustGetAgentActivityResponse::IncompleteChain;
                    }
                }

                // The constraints are met the activity can be returned.
                MustGetAgentActivityResponse::Activity(out)
            }
            // The constraints are not met so the chain is not complete.
            _ => MustGetAgentActivityResponse::IncompleteChain,
        }
    }
}
