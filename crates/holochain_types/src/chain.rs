//! Types related to an agents for chain activity
use std::iter::Peekable;
use std::ops::RangeInclusive;

use crate::activity::AgentActivityResponse;
use crate::activity::ChainItems;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_zome_types::prelude::ChainStatus;
use holochain_zome_types::ChainFilter;
use holochain_zome_types::ChainFilters;
use holochain_zome_types::RegisterAgentActivity;

#[cfg(all(test, feature = "test_utils"))]
mod test;

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

#[warn(missing_docs)]
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
pub struct ChainFilterIter {
    filter: ChainFilter,
    iter: Peekable<std::vec::IntoIter<RegisterAgentActivity>>,
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
    /// The start of the ranges type.
    range_start_type: RangeStartType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The type of chain item that starts the range.
enum RangeStartType {
    /// The range starts from genesis.
    Genesis,
    /// The Range starts from an action where `take`
    /// has reached zero.
    Take,
    /// The range starts from an action where an
    /// `until` hash was found.
    Until,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Outcome of trying to find the action sequences in a filter.
pub enum Sequences {
    /// Found all action sequences
    Found(ChainFilterRange),
    /// The following action was not found.
    ActionNotFound(ActionHash),
    /// The starting position is not the highest
    /// sequence in the filter.
    PositionNotHighest,
    /// The filter produces an empty range.
    EmptyRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Constraints on a chain filter.
/// This is used to check the following
/// invariants are upheld after filtering
/// a chain.
/// - The result starts with the position.
/// - The result ends with minimum of:
///   - `position - take`.
///   - minimum of until hashes.
/// - If there are no filters then the
/// result should end in 0 (genesis).
pub struct ChainFilterConstraints {
    filter: ChainFilter,
    until: Option<UntilConstraint>,
    range: RangeInclusive<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The chain must end with this `until` hash.
pub struct UntilConstraint {
    /// The "until" hash with the highest action sequence.
    pub hash: ActionHash,
    /// The sequence of the above hash.
    pub action_seq: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Response to a `must_get_agent_activity` call.
pub enum MustGetAgentActivityResponse {
    /// The activity was found.
    Activity(Vec<RegisterAgentActivity>),
    /// The requested chain range was incomplete.
    IncompleteChain,
    /// The requested action was not found in the chain.
    ActionNotFound(ActionHash),
    /// The starting position is not the highest
    /// sequence in the filter.
    PositionNotHighest,
    /// The filter produces an empty range.
    EmptyRange,
}

impl ChainFilterIter {
    /// Create an iterator that filters an iterator of [`RegisterAgentActivity`]
    /// with a [`ChainFilter`].
    ///
    /// # Constraints
    /// - The input iterator **must** be sorted by action sequence from highest to lowest.
    /// - If the input iterator is not sorted by action sequence
    /// then this iterator will only work on the first sorted subset.
    /// - If the iterator does not contain the filters starting position
    /// then this will be an empty iterator.
    pub fn new(filter: ChainFilter, mut chain: Vec<RegisterAgentActivity>) -> Self {
        // Sort by descending.
        chain.sort_unstable_by(|a, b| {
            b.action
                .action()
                .action_seq()
                .cmp(&a.action.action().action_seq())
        });
        // Create a peekable iterator.
        let mut iter = chain.into_iter().peekable();

        // Discard any ops that are not the starting position.
        let i = iter.by_ref();
        while let Some(op) = i.peek() {
            if *op.action.action_address() == filter.position {
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

impl Iterator for ChainFilterIter {
    type Item = RegisterAgentActivity;

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
                    let child_seq = op.action.hashed.action_seq();
                    let parent_seq = parent.action.hashed.action_seq();
                    match (child_seq.cmp(&parent_seq), op.action.hashed.prev_action()) {
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
                            if prev_hash != parent.action.action_address() =>
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
                if until_hashes.contains(op.action.action_address()) {
                    // If it has, include it and return on the next call to `next`.
                    self.end = true;
                }
            }
            // Just keep going till genesis.
            ChainFilters::ToGenesis => (),
            // Both filters are active. Return on the first to be hit.
            ChainFilters::Both(n, until_hashes) => {
                *n = n.checked_sub(1)?;

                if until_hashes.contains(op.action.action_address()) {
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
        let position = match get_seq(&filter.position)? {
            Some(seq) => seq,
            None => return Ok(Self::ActionNotFound(filter.position)),
        };
        let mut range_start_type = RangeStartType::Genesis;
        let distance = match filter.get_until() {
            Some(until_hashes) => {
                range_start_type = RangeStartType::Until;
                let mut max = 0;
                for hash in until_hashes {
                    match get_seq(hash)? {
                        Some(seq) => {
                            if seq > position {
                                return Ok(Self::PositionNotHighest);
                            }
                            max = max.max(seq);
                        }
                        None => return Ok(Self::ActionNotFound(hash.clone())),
                    }
                }
                position - max
            }
            None => position,
        };
        let start = match filter.get_take() {
            Some(take) => {
                if take == 0 {
                    return Ok(Self::EmptyRange);
                } else if take <= distance {
                    range_start_type = RangeStartType::Take;
                    // Add one to include the "position" in the number of
                    // "take". This matches the rust iterator "take".
                    position.saturating_sub(take).saturating_add(1)
                } else {
                    position - distance
                }
            }
            None => position - distance,
        };
        Ok(Self::Found(ChainFilterRange {
            filter,
            range: start..=position,
            range_start_type,
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
        let out: Vec<_> = ChainFilterIter::new(self.filter, chain).collect();
        match out.last().zip(out.first()) {
            Some((lowest, highest))
                if (lowest.action.action().action_seq()..=highest.action.action().action_seq())
                    == self.range =>
            {
                // Check if the range start is an until action sequence that it is
                // actually an until hash.
                if let Some(hashes) = until_hashes {
                    if matches!(self.range_start_type, RangeStartType::Until)
                        && !hashes.contains(lowest.action.action_address())
                    {
                        return MustGetAgentActivityResponse::IncompleteChain;
                    }
                }
                MustGetAgentActivityResponse::Activity(out)
            }
            _ => MustGetAgentActivityResponse::IncompleteChain,
        }
    }
}
