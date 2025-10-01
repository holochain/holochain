//! Types related to an agents for chain activity
use crate::activity::AgentActivityResponse;
use crate::activity::ChainItems;
use crate::warrant::WarrantOp;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::HasHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;
use std::collections::BTreeSet;
use std::iter::Peekable;
use std::ops::RangeInclusive;

#[cfg(all(test, feature = "test_utils"))]
mod test;

mod chain_item;
pub use chain_item::*;

/// Helpers for constructing AgentActivity
pub trait AgentActivityExt {
    /// Create an empty chain status
    fn empty<T>(agent: &AgentPubKey) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
            warrants: vec![],
        }
    }
}

impl AgentActivityExt for AgentActivityResponse {}

/// Iterate over a source chain and apply the [`ChainFilter`] to each element.
/// This iterator will:
/// - Ignore any ops that are not a direct ancestor to the starting position.
/// - Stop at the first gap in the chain.
/// - Take no **more** then the [`take`]. It may return less.
/// - Stop at (including) the [`ActionHash`] in [`until_hash`]. But not if this hash is not in the chain.
/// - Stop when [`until_timestamp`] is reached.
///
/// [`take`]: ChainFilter::take
/// [`until_timestamp`]: ChainFilter::until_timestamp
/// [`until_hash`]: ChainFilter::until_hash
#[must_use = "Iterator doesn't do anything unless consumed."]
#[derive(Debug)]
pub struct ChainFilterIter<I: AsRef<A>, A: ChainItem = SignedActionHashed> {
    filter: ChainFilter<A::Hash>,
    iter: Peekable<std::vec::IntoIter<I>>,
    end: bool,
}

/// A [`ChainFilter`] with the action sequences for the
/// starting position and any `until` hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainFilterRange {
    /// The filter for this chain.
    filter: ChainFilter,
    /// The start of this range is the end of
    /// the filter iterator.
    /// The end of this range is the sequence of
    /// the starting position hash.
    range: RangeInclusive<u32>,
    /// The start of the range's type.
    chain_bottom_type: ChainBottomType,
}

/// The type of chain item that forms the bottom of the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChainBottomType {
    /// The bottom of the chain is genesis.
    Genesis,
    /// The bottom of the chain is the action where `take`
    /// has reached zero.
    Take,
    /// The bottom of the chain is the oldest action before
    /// `until_timestamp` is reached.
    UntilTimestamp,
    /// The bottom of the chain is the action where an
    /// `until_hash` hash was found.
    UntilHash,
}

/// Outcome of trying to find the action sequences in a filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sequences {
    /// Found all action sequences
    Found(ChainFilterRange),
    /// The chain top action was not found.
    ChainTopNotFound(ActionHash),
    /// The filter produces an empty range.
    EmptyRange,
}

/// Intermediate data structure used during a `must_get_agent_activity` call.
/// Note that this is not the final return value of `must_get_agent_activity`.
#[derive(Debug, Clone, PartialEq, Eq, SerializedBytes, Serialize, Deserialize)]
pub enum MustGetAgentActivityResponse {
    /// The activity was found.
    Activity {
        /// The actions performed by the agent.
        activity: Vec<RegisterAgentActivity>,
        /// Any warrants issued to the agent for this activity.
        warrants: Vec<WarrantOp>,
    },
    /// The requested chain range was incomplete.
    IncompleteChain,
    /// The requested chain top was not found in the chain.
    ChainTopNotFound(ActionHash),
    /// The filter produces an empty range.
    EmptyRange,
}

impl MustGetAgentActivityResponse {
    /// Constructor
    #[cfg(feature = "test_utils")]
    pub fn activity(activity: Vec<RegisterAgentActivity>) -> Self {
        Self::Activity {
            activity,
            warrants: vec![],
        }
    }
}

/// Identical structure to [`MustGetAgentActivityResponse`] except it includes
/// the [`ChainFilterRange`] that was used to produce the response. Doesn't need
/// to be serialized because it is only used internally.
/// Note that this is not the final return value of `must_get_agent_activity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedMustGetAgentActivityResponse {
    /// The activity was found.
    Activity {
        /// The actions performed by the agent.
        activity: Vec<RegisterAgentActivity>,
        /// Any warrants issued to the agent for this activity.
        warrants: Vec<WarrantOp>,
        /// The filter used to produce this response.
        filter: ChainFilterRange,
    },
    /// The requested chain range was incomplete.
    IncompleteChain,
    /// The requested chain top was not found in the chain.
    ChainTopNotFound(ActionHash),
    /// The filter produces an empty range.
    EmptyRange,
}

impl BoundedMustGetAgentActivityResponse {
    /// Sort by the chain seq.
    /// Dedupe by action hash.
    pub fn normalize(&mut self) {
        if let Self::Activity { activity, .. } = self {
            activity.sort_unstable_by_key(|a| a.action.action().action_seq());
            activity.dedup_by_key(|a| a.action.as_hash().clone());
        }
    }

    /// Constructor
    #[cfg(feature = "test_utils")]
    pub fn activity(actions: Vec<RegisterAgentActivity>, filter: ChainFilterRange) -> Self {
        Self::Activity {
            activity: actions,
            filter,
            warrants: vec![],
        }
    }
}

impl From<BoundedMustGetAgentActivityResponse> for MustGetAgentActivityResponse {
    fn from(val: BoundedMustGetAgentActivityResponse) -> Self {
        match val {
            BoundedMustGetAgentActivityResponse::Activity {
                activity,
                filter,
                warrants,
            } => {
                // Check if we are missing any action seqs within the target filter range
                let first_action = activity
                    .first()
                    .expect("MustGetAgentActivityResponse::Activity must contain activity");
                let last_action = activity
                    .last()
                    .expect("MustGetAgentActivityResponse::Activity must contain activity");

                println!("first action {}", first_action.action.action().action_seq());
                println!("last action {}", last_action.action.action().action_seq());

                let target_action_seqs: BTreeSet<u32> =
                    filter.range().clone().collect();
                let held_action_seqs: BTreeSet<u32> = (first_action.action.action().action_seq()
                    ..=last_action.action.action().action_seq())
                    .collect();
                let missing_action_seqs: BTreeSet<u32> = held_action_seqs
                    .difference(&target_action_seqs)
                    .cloned()
                    .collect();

                if missing_action_seqs.is_empty() {
                    MustGetAgentActivityResponse::Activity { activity, warrants }
                } else {
                    MustGetAgentActivityResponse::IncompleteChain
                }
            }
            BoundedMustGetAgentActivityResponse::IncompleteChain => {
                MustGetAgentActivityResponse::IncompleteChain
            }
            BoundedMustGetAgentActivityResponse::ChainTopNotFound(a) => {
                MustGetAgentActivityResponse::ChainTopNotFound(a)
            }
            BoundedMustGetAgentActivityResponse::EmptyRange => {
                MustGetAgentActivityResponse::EmptyRange
            }
        }
    }
}

/// Merges two agent activity responses, along with their chain filters if
/// present. Chain filter range mismatches are treated as an incomplete
/// chain for the purpose of merging. Merging should only be done on
/// responses that originate from the same authority, so the chain filters
/// should always match, or at least their mismatch is the responsibility of
/// a single authority.
pub fn merge_bounded_agent_activity_responses(
    responses: Vec<BoundedMustGetAgentActivityResponse>,
) -> BoundedMustGetAgentActivityResponse {
    let any_responses_empty_chain = responses
        .iter()
        .any(|r| matches!(r, BoundedMustGetAgentActivityResponse::EmptyRange));
    let any_responses_activity = responses
        .iter()
        .any(|r| matches!(r, BoundedMustGetAgentActivityResponse::Activity { .. }));
    let any_responses_chain_incomplete_chain = responses
        .iter()
        .any(|r| matches!(r, BoundedMustGetAgentActivityResponse::IncompleteChain));
    let all_responses_chain_top_not_found = responses
        .iter()
        .all(|r| matches!(r, BoundedMustGetAgentActivityResponse::ChainTopNotFound(_)));

    // If any responses are Activity, return Activity,
    // merging the activity data.
    if any_responses_activity {
        let mut merged_activity = vec![];
        let mut merged_warrants = vec![];
        let mut chain_filter_ranges = vec![];

        // Merge all Activity responses received
        for r in responses {
            if let BoundedMustGetAgentActivityResponse::Activity {
                mut activity,
                mut warrants,
                filter,
            } = r
            {
                merged_activity.append(&mut activity);
                merged_warrants.append(&mut warrants);
                chain_filter_ranges.push(filter);
            };
        }

        // Sort activity by action seq ascending, to match the sql query
        // which generates each individual BoundedMustGetAgentActivityResponse
        merged_activity.sort_unstable_by_key(|a| a.action.action().action_seq());

        // Verify chain filters in all Activity responses are equivalent
        //
        // Non-equivalent chain filters indicates a bug or invalid behavior,
        // and should never occur.
        if let Some(first_chain_filter_range) = chain_filter_ranges.first() {
            let chain_filters_match = chain_filter_ranges
                .iter()
                .all(|r| r == first_chain_filter_range);
            if !chain_filters_match {
                tracing::error!(
                    "ChainFilterRange do not match for the same bounded agent activity query"
                );
                return BoundedMustGetAgentActivityResponse::IncompleteChain;
            }

            // Sort & return merged Activity
            let mut merged_response = BoundedMustGetAgentActivityResponse::Activity {
                activity: merged_activity,
                warrants: merged_warrants,
                filter: first_chain_filter_range.clone(),
            };
            merged_response.normalize();

            return merged_response;
        }
    }
    // If any responses are EmptyRange, return EmptyRange
    else if any_responses_empty_chain {
        return BoundedMustGetAgentActivityResponse::EmptyRange;
    }
    // If any responses are IncompleteChain, return IncompleteChain
    else if any_responses_chain_incomplete_chain {
        return BoundedMustGetAgentActivityResponse::IncompleteChain;
    }
    // If all responses are ChainTopNotFound, return ChainTopNotFound
    else if all_responses_chain_top_not_found {
        if let Some(first_response) = responses.first() {
            return first_response.clone();
        }
    }

    // Otherwise, return IncompleteChain
    BoundedMustGetAgentActivityResponse::IncompleteChain
}

impl<I: AsRef<A>, A: ChainItem> ChainFilterIter<I, A> {
    /// Create an iterator that filters an iterator of actions
    /// with a [`ChainFilter`].
    ///
    /// # Constraints
    /// - If the iterator does not contain the filter's chain_top
    ///   then this will be an empty iterator.
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

        match &mut self.filter.limit_conditions {
            // Check if there is any left to take.
            LimitConditions::Take(n) => *n = n.checked_sub(1)?,
            // Check if the timestamp has been passed.
            LimitConditions::UntilTimestamp(ts) => {
                if op.as_ref().get_timestamp() < *ts {
                    // If timestamp has been passed, end search and don't return this item.
                    self.end = true;
                    return None;
                }
            }
            // Check if the `until_hash` hash has been found.
            LimitConditions::UntilHash(until_hashes) => {
                if until_hashes.contains(op.as_ref().get_hash()) {
                    // If it has, include it and return on the next call to `next`.
                    self.end = true;
                }
            }
            // Just keep going till genesis.
            LimitConditions::ToGenesis => (),
            // Both filters are active. Return on the first to be hit.
            LimitConditions::Multiple(maybe_n, until_hashes, maybe_ts) => {
                if let Some(n) = maybe_n {
                    *n = n.checked_sub(1)?;
                }

                if until_hashes.contains(op.as_ref().get_hash()) {
                    self.end = true;
                }

                if let Some(ts) = maybe_ts {
                    if op.as_ref().get_timestamp() < *ts {
                        // Timestamp passed, don't return item.
                        self.end = true;
                        return None;
                    }
                }
            }
        }
        Some(op)
    }
}

impl Sequences {
    /// Find the action sequences for all hashes in the filter.
    pub fn find_sequences<F, F2, E>(
        filter: ChainFilter,
        mut get_seq_from_hash: F,
        mut get_seq_from_ts: F2,
    ) -> Result<Self, E>
    where
        F: FnMut(&ActionHash) -> Result<Option<u32>, E>,
        F2: FnMut(Timestamp) -> Result<Option<u32>, E>,
    {
        // Get the top of the chain action sequence.
        // This is the highest sequence number and also the
        // start of the iterator.
        let chain_top = match get_seq_from_hash(&filter.chain_top)? {
            Some(seq) => seq,
            None => return Ok(Self::ChainTopNotFound(filter.chain_top)),
        };

        // Track how the sequence start of the range was chosen.
        let mut chain_bottom_type = ChainBottomType::Genesis;

        // If there are any until hash conditions in the filter,
        // then find the highest sequence of the set
        // and find the distance from the position.
        let mut distance = match filter.get_until_hash() {
            Some(until_hashes) => {
                // Find the highest sequence of the until hashes.
                let max = until_hashes
                    .iter()
                    .filter_map(|hash| {
                        match get_seq_from_hash(hash) {
                            Ok(seq) => {
                                // Ignore any until hashes that could not be found.
                                let seq = seq?;
                                // Ignore any until hashes that are higher than the chain top.
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
                    chain_bottom_type = ChainBottomType::UntilHash;
                }

                // The distance from the chain top till highest until hash.
                // Note this cannot be an overflow due to the check above.
                chain_top - max
            }
            // If there is no until hashes then the distance is the chain top
            // till genesis (or just the chain top).
            None => chain_top,
        };

        // Check if there is an until timestamp condition in the filter
        // and get the distance from the position.
        if let Some(until_ts) = filter.get_until_timestamp() {
            if let Some(seq) = get_seq_from_ts(until_ts)? {
                let ts_distance = chain_top - seq;
                // Keep the shortest distance between untilHash and untilTimestamp.
                if ts_distance <= distance {
                    chain_bottom_type = ChainBottomType::UntilTimestamp;
                    distance = ts_distance;
                }
            }
        }

        // Check if there is a take condition in the filter and if that
        // will be reached before any other condition.
        let start = match filter.get_take() {
            Some(take) => {
                // A take of zero will produce an empty range.
                if take == 0 {
                    return Ok(Self::EmptyRange);
                } else if take <= distance {
                    // The take limit will be reached first.
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
}

#[cfg(test)]
mod tests {
    use super::BoundedMustGetAgentActivityResponse;
    use super::ChainBottomType;
    use super::ChainFilter;
    use super::ChainFilterRange;
    use holochain_types::prelude::*;
    use test_case::test_case;

    /// If both sides are not activity then the acc should be returned.
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::EmptyRange,
            BoundedMustGetAgentActivityResponse::EmptyRange
        ]
        => BoundedMustGetAgentActivityResponse::EmptyRange
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::EmptyRange,
        BoundedMustGetAgentActivityResponse::IncompleteChain]
        => BoundedMustGetAgentActivityResponse::EmptyRange
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::EmptyRange,
        BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36]))]
        => BoundedMustGetAgentActivityResponse::EmptyRange
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::IncompleteChain,
        BoundedMustGetAgentActivityResponse::IncompleteChain]
        => BoundedMustGetAgentActivityResponse::IncompleteChain
    )]
    // This seems like the opposite of the previous test
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::IncompleteChain,
        BoundedMustGetAgentActivityResponse::EmptyRange]
        => BoundedMustGetAgentActivityResponse::EmptyRange
    )]
    // What is the difference between IncompleteChain and ChainTopNotFound
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::IncompleteChain,
        BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36]))]
        => BoundedMustGetAgentActivityResponse::IncompleteChain
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36])),
        BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![1; 36]))]
        => BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36]))
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36])),
        BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36]))]
        => BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36]))
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36])),
        BoundedMustGetAgentActivityResponse::EmptyRange]
        => BoundedMustGetAgentActivityResponse::EmptyRange
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36])),
        BoundedMustGetAgentActivityResponse::IncompleteChain]
        => BoundedMustGetAgentActivityResponse::IncompleteChain
    )]
    /// If one side is activity and the other is not then the activity should be returned.
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        }),
        BoundedMustGetAgentActivityResponse::EmptyRange]
        => BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        }),
        BoundedMustGetAgentActivityResponse::IncompleteChain]
        => BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        }),
        BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36]))]
        => BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::EmptyRange,
        BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })]
        => BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::IncompleteChain,
        BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })]
        => BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::ChainTopNotFound(ActionHash::from_raw_36(vec![0; 36])),
        BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })]
        => BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]
    /// If both sides are activity then the activity should be merged.
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        }),
        BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })]
        => BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]
    #[test_case(
        vec![
            BoundedMustGetAgentActivityResponse::activity(vec![], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        }),
        BoundedMustGetAgentActivityResponse::activity(vec![RegisterAgentActivity{
            action: SignedActionHashed::with_presigned(
                ActionHashed::from_content_sync(Action::Dna(Dna {
                    author: AgentPubKey::from_raw_36(vec![0; 36]),
                    timestamp: Timestamp(0),
                    hash: DnaHash::from_raw_36(vec![0; 36]),
                })),
                Signature([0; SIGNATURE_BYTES]),
            ),
            cached_entry: None,
        }], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
        ]
        => BoundedMustGetAgentActivityResponse::activity(vec![RegisterAgentActivity{
            action: SignedActionHashed::with_presigned(
                ActionHashed::from_content_sync(Action::Dna(Dna {
                    author: AgentPubKey::from_raw_36(vec![0; 36]),
                    timestamp: Timestamp(0),
                    hash: DnaHash::from_raw_36(vec![0; 36]),
                })),
                Signature([0; SIGNATURE_BYTES]),
            ),
            cached_entry: None
        }], ChainFilterRange {
            filter: ChainFilter::new(ActionHash::from_raw_36(vec![0; 36])),
            range: 0..=0,
            chain_bottom_type: ChainBottomType::Genesis,
        })
    )]

    fn test_merge_bounded_agent_activity_responses(
        responses: Vec<BoundedMustGetAgentActivityResponse>,
    ) -> BoundedMustGetAgentActivityResponse {
        super::merge_bounded_agent_activity_responses(responses)
    }
}
