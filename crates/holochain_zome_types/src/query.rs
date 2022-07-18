//! Types for source chain queries

use std::collections::HashMap;
use std::collections::HashSet;

use crate::action::ActionType;
use crate::action::EntryType;
use crate::warrant::Warrant;
use crate::ActionHashed;
use crate::Record;
use holo_hash::ActionHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
pub use holochain_serialized_bytes::prelude::*;

/// Defines several ways that queries can be restricted to a range.
/// Notably hash bounded ranges disambiguate forks whereas sequence indexes do
/// not as the same position can be found in many forks.
/// The reason that this does NOT use native rust range traits is that the hash
/// bounded queries MUST be inclusive otherwise the integrity and fork
/// disambiguation logic is impossible. An exclusive range bound that does not
/// include the final action tells us nothing about which fork to select
/// between N forks of equal length that proceed it. With an inclusive hash
/// bounded range the final action always points unambiguously at the "correct"
/// fork that the range is over. Start hashes are not needed to provide this
/// property so ranges can be hash terminated with a length of preceeding
/// records to return only. Technically the seq bounded ranges do not imply
/// any fork disambiguation and so could be a range but for simplicity we left
/// the API symmetrical in boundedness across all enum variants.
/// @TODO It may be possible to provide/implement RangeBounds in the case that
/// a full sequence of records/actions is provided but it would need to be
/// handled as inclusive first, to enforce the integrity of the query, then the
/// exclusiveness achieved by simply removing the final record after the fact.
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Clone, Debug)]
pub enum ChainQueryFilterRange {
    /// Do NOT apply any range filtering for this query.
    Unbounded,
    /// A range over source chain sequence numbers.
    /// This is ambiguous over forking histories and so should NOT be used in
    /// validation logic.
    /// Inclusive start, inclusive end.
    ActionSeqRange(u32, u32),
    /// A range over source chain action hashes.
    /// This CAN be used in validation logic as forks are disambiguated.
    /// Inclusive start and end (unlike std::ops::Range).
    ActionHashRange(ActionHash, ActionHash),
    /// The terminating action hash and N preceeding records.
    /// N = 0 returns only the record with this `ActionHash`.
    /// This CAN be used in validation logic as forks are not possible when
    /// "looking up" towards genesis from some `ActionHash`.
    ActionHashTerminated(ActionHash, u32),
}

impl Default for ChainQueryFilterRange {
    fn default() -> Self {
        Self::Unbounded
    }
}

/// Query arguments
#[derive(
    serde::Serialize, serde::Deserialize, SerializedBytes, Default, PartialEq, Clone, Debug,
)]
// TODO: get feedback on whether it's OK to remove non_exhaustive
// #[non_exhaustive]
pub struct ChainQueryFilter {
    /// Limit the results to a range of records according to their actions.
    pub sequence_range: ChainQueryFilterRange,
    /// Filter by EntryType
    // NB: if this filter is set, you can't verify the results, so don't
    //     use this in validation
    pub entry_type: Option<EntryType>,
    /// Filter by a list of `EntryHash`.
    pub entry_hashes: Option<HashSet<EntryHash>>,
    /// Filter by ActionType
    // NB: if this filter is set, you can't verify the results, so don't
    //     use this in validation
    pub action_type: Option<ActionType>,
    /// Include the entries in the records
    pub include_entries: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// An agents chain records returned from a agent_activity_query
pub struct AgentActivity {
    /// Valid actions on this chain.
    pub valid_activity: Vec<(u32, ActionHash)>,
    /// Rejected actions on this chain.
    pub rejected_activity: Vec<(u32, ActionHash)>,
    /// The status of this chain.
    pub status: ChainStatus,
    /// The highest chain action that has
    /// been observed by this authority.
    pub highest_observed: Option<HighestObserved>,
    /// Warrants about this AgentActivity.
    /// Placeholder for future.
    pub warrants: Vec<Warrant>,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// Get either the full activity or just the status of the chain
pub enum ActivityRequest {
    /// Just request the status of the chain
    Status,
    /// Request all the activity
    Full,
}

#[derive(Clone, Debug, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
/// The highest action sequence observed by this authority.
/// This also includes the actions at this sequence.
/// If there is more then one then there is a fork.
///
/// This type is to prevent actions being hidden by
/// withholding the previous action.
///
/// The information is tracked at the edge of holochain before
/// validation (but after drop checks).
pub struct HighestObserved {
    /// The highest sequence number observed.
    pub action_seq: u32,
    /// Hashes of any actions claiming to be at this
    /// action sequence.
    pub hash: Vec<ActionHash>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// Status of the agent activity chain
// TODO: In the future we will most likely be replaced
// by warrants instead of Forked / Invalid so we can provide
// evidence of why the chain has a status.
pub enum ChainStatus {
    /// This authority has no information on the chain.
    Empty,
    /// The chain is valid as at this action sequence and action hash.
    Valid(ChainHead),
    /// Chain is forked.
    Forked(ChainFork),
    /// Chain is invalid because of this action.
    Invalid(ChainHead),
}

impl Default for ChainStatus {
    fn default() -> Self {
        ChainStatus::Empty
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// The action at the head of the complete chain.
/// This is as far as this authority can see a
/// chain with no gaps.
pub struct ChainHead {
    /// Sequence number of this chain head.
    pub action_seq: u32,
    /// Hash of this chain head
    pub hash: ActionHash,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// The chain has been forked by these two actions
pub struct ChainFork {
    /// The point where the chain has forked.
    pub fork_seq: u32,
    /// The first action at this sequence position.
    pub first_action: ActionHash,
    /// The second action at this sequence position.
    pub second_action: ActionHash,
}

impl ChainQueryFilter {
    /// Create a no-op ChainQueryFilter which returns everything.
    pub fn new() -> Self {
        Self {
            include_entries: false,
            ..Self::default()
        }
    }

    /// Filter on sequence range.
    pub fn sequence_range(mut self, sequence_range: ChainQueryFilterRange) -> Self {
        self.sequence_range = sequence_range;
        self
    }

    /// Filter on entry type.
    pub fn entry_type(mut self, entry_type: EntryType) -> Self {
        self.entry_type = Some(entry_type);
        self
    }

    /// Filter on entry hashes.
    pub fn entry_hashes(mut self, entry_hashes: HashSet<EntryHash>) -> Self {
        self.entry_hashes = Some(entry_hashes);
        self
    }

    /// Filter on action type.
    pub fn action_type(mut self, action_type: ActionType) -> Self {
        self.action_type = Some(action_type);
        self
    }

    /// Include the entries in the RecordsVec that is returned.
    pub fn include_entries(mut self, include_entries: bool) -> Self {
        self.include_entries = include_entries;
        self
    }

    /// If the sequence range supports fork disambiguation, apply it to remove
    /// actions that are not in the correct branch.
    /// Numerical range bounds do NOT support fork disambiguation, and neither
    /// does unbounded, but everything hash bounded does.
    pub fn disambiguate_forks(&self, actions: Vec<ActionHashed>) -> Vec<ActionHashed> {
        match &self.sequence_range {
            ChainQueryFilterRange::Unbounded => actions,
            ChainQueryFilterRange::ActionSeqRange(start, end) => actions
                .into_iter()
                .filter(|action| *start <= action.action_seq() && action.action_seq() <= *end)
                .collect(),
            ChainQueryFilterRange::ActionHashRange(start, end) => {
                let mut action_hashmap = actions
                    .into_iter()
                    .map(|action| (action.as_hash().clone(), action))
                    .collect::<HashMap<ActionHash, ActionHashed>>();
                let mut filtered_actions = Vec::new();
                let mut maybe_next_action = action_hashmap.remove(end);
                while let Some(next_action) = maybe_next_action {
                    maybe_next_action = next_action
                        .as_content()
                        .prev_action()
                        .and_then(|prev_action| action_hashmap.remove(prev_action));
                    let is_start = next_action.as_hash() == start;
                    filtered_actions.push(next_action);
                    // This comes after the push to make the range inclusive.
                    if is_start {
                        break;
                    }
                }
                filtered_actions
            }
            ChainQueryFilterRange::ActionHashTerminated(end, n) => {
                let mut action_hashmap = actions
                    .iter()
                    .map(|action| (action.as_hash().clone(), action))
                    .collect::<HashMap<ActionHash, &ActionHashed>>();
                let mut filtered_actions = Vec::new();
                let mut maybe_next_action = action_hashmap.remove(end);
                let mut i = 0;
                while let Some(next_action) = maybe_next_action {
                    maybe_next_action = next_action
                        .as_content()
                        .prev_action()
                        .and_then(|prev_action| action_hashmap.remove(prev_action));
                    filtered_actions.push(next_action.clone());
                    // This comes after the push to make the range inclusive.
                    if i == *n {
                        break;
                    }
                    i += 1;
                }
                filtered_actions
            }
        }
    }

    /// Filter a vector of hashed actions according to the query.
    pub fn filter_actions(&self, actions: Vec<ActionHashed>) -> Vec<ActionHashed> {
        self.disambiguate_forks(actions)
            .into_iter()
            .filter(|action| {
                self.action_type
                    .as_ref()
                    .map(|action_type| action.action_type() == *action_type)
                    .unwrap_or(true)
                    && self
                        .entry_type
                        .as_ref()
                        .map(|entry_type| action.entry_type() == Some(entry_type))
                        .unwrap_or(true)
                    && self
                        .entry_hashes
                        .as_ref()
                        .map(|entry_hashes| match action.entry_hash() {
                            Some(entry_hash) => entry_hashes.contains(entry_hash),
                            None => false,
                        })
                        .unwrap_or(true)
            })
            .collect()
    }

    /// Filter a vector of records according to the query.
    pub fn filter_records(&self, records: Vec<Record>) -> Vec<Record> {
        let actions = self.filter_actions(
            records
                .iter()
                .map(|record| record.action_hashed().clone())
                .collect(),
        );
        let action_hashset = actions
            .iter()
            .map(|action| action.as_hash().clone())
            .collect::<HashSet<ActionHash>>();
        records
            .into_iter()
            .filter(|record| action_hashset.contains(record.action_address()))
            .collect()
    }
}

#[cfg(test)]
#[cfg(feature = "fixturators")]
mod tests {
    use super::ChainQueryFilter;
    use crate::action::EntryType;
    use crate::fixt::AppEntryTypeFixturator;
    use crate::fixt::*;
    use crate::ActionHashed;
    use crate::ChainQueryFilterRange;
    use ::fixt::prelude::*;
    use holo_hash::HasHash;

    /// Create three Actions with various properties.
    /// Also return the EntryTypes used to construct the first two actions.
    fn fixtures() -> [ActionHashed; 7] {
        let entry_type_1 = EntryType::App(fixt!(AppEntryType));
        let entry_type_2 = EntryType::AgentPubKey;

        let entry_hash_0 = fixt!(EntryHash);

        let mut h0 = fixt!(Create);
        h0.entry_type = entry_type_1.clone();
        h0.action_seq = 0;
        h0.entry_hash = entry_hash_0.clone();
        let hh0 = ActionHashed::from_content_sync(h0.into());

        let mut h1 = fixt!(Update);
        h1.entry_type = entry_type_2.clone();
        h1.action_seq = 1;
        h1.prev_action = hh0.as_hash().clone();
        let hh1 = ActionHashed::from_content_sync(h1.into());

        let mut h2 = fixt!(CreateLink);
        h2.action_seq = 2;
        h2.prev_action = hh1.as_hash().clone();
        let hh2 = ActionHashed::from_content_sync(h2.into());

        let mut h3 = fixt!(Create);
        h3.entry_type = entry_type_2.clone();
        h3.action_seq = 3;
        h3.prev_action = hh2.as_hash().clone();
        let hh3 = ActionHashed::from_content_sync(h3.into());

        // Cheeky forker!
        let mut h3a = fixt!(Create);
        h3a.entry_type = entry_type_1.clone();
        h3a.action_seq = 3;
        h3a.prev_action = hh2.as_hash().clone();
        let hh3a = ActionHashed::from_content_sync(h3a.into());

        let mut h4 = fixt!(Update);
        h4.entry_type = entry_type_1.clone();
        // same entry content as h0
        h4.entry_hash = entry_hash_0;
        h4.action_seq = 4;
        h4.prev_action = hh3.as_hash().clone();
        let hh4 = ActionHashed::from_content_sync(h4.into());

        let mut h5 = fixt!(CreateLink);
        h5.action_seq = 5;
        h5.prev_action = hh4.as_hash().clone();
        let hh5 = ActionHashed::from_content_sync(h5.into());

        let actions = [hh0, hh1, hh2, hh3, hh3a, hh4, hh5];
        actions
    }

    fn map_query(query: &ChainQueryFilter, actions: &[ActionHashed]) -> Vec<bool> {
        let filtered = query.filter_actions(actions.to_vec());
        actions
            .iter()
            .map(|h| filtered.contains(h))
            .collect::<Vec<_>>()
    }

    #[test]
    fn filter_by_entry_type() {
        let actions = fixtures();

        let query_1 =
            ChainQueryFilter::new().entry_type(actions[0].entry_type().unwrap().to_owned());
        let query_2 =
            ChainQueryFilter::new().entry_type(actions[1].entry_type().unwrap().to_owned());

        assert_eq!(
            map_query(&query_1, &actions),
            [true, false, false, false, true, true, false].to_vec()
        );
        assert_eq!(
            map_query(&query_2, &actions),
            [false, true, false, true, false, false, false].to_vec()
        );
    }

    #[test]
    fn filter_by_entry_hash() {
        let actions = fixtures();

        let query = ChainQueryFilter::new().entry_hashes(
            vec![
                actions[3].entry_hash().unwrap().clone(),
                // actions[5] has same entry hash as actions[0]
                actions[5].entry_hash().unwrap().clone(),
            ]
            .into_iter()
            .collect(),
        );

        assert_eq!(
            map_query(&query, &actions),
            vec![true, false, false, true, false, true, false]
        );
    }

    #[test]
    fn filter_by_action_type() {
        let actions = fixtures();

        let query_1 = ChainQueryFilter::new().action_type(actions[0].action_type());
        let query_2 = ChainQueryFilter::new().action_type(actions[1].action_type());
        let query_3 = ChainQueryFilter::new().action_type(actions[2].action_type());

        assert_eq!(
            map_query(&query_1, &actions),
            [true, false, false, true, true, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_2, &actions),
            [false, true, false, false, false, true, false].to_vec()
        );
        assert_eq!(
            map_query(&query_3, &actions),
            [false, false, true, false, false, false, true].to_vec()
        );
    }

    #[test]
    fn filter_by_chain_sequence() {
        let actions = fixtures();

        for (sequence_range, expected, name) in vec![
            (
                ChainQueryFilterRange::Unbounded,
                vec![true, true, true, true, true, true, true],
                "unbounded",
            ),
            (
                ChainQueryFilterRange::ActionSeqRange(0, 0),
                vec![true, false, false, false, false, false, false],
                "first only",
            ),
            (
                ChainQueryFilterRange::ActionSeqRange(0, 1),
                vec![true, true, false, false, false, false, false],
                "several from start",
            ),
            (
                ChainQueryFilterRange::ActionSeqRange(1, 2),
                vec![false, true, true, false, false, false, false],
                "several not start",
            ),
            (
                ChainQueryFilterRange::ActionSeqRange(2, 999),
                vec![false, false, true, true, true, true, true],
                "exceeds chain length, not start",
            ),
            (
                ChainQueryFilterRange::ActionHashRange(
                    actions[2].as_hash().clone(),
                    actions[6].as_hash().clone(),
                ),
                vec![false, false, true, true, false, true, true],
                "hash bounded not 3a",
            ),
            (
                ChainQueryFilterRange::ActionHashRange(
                    actions[2].as_hash().clone(),
                    actions[4].as_hash().clone(),
                ),
                vec![false, false, true, false, true, false, false],
                "hash bounded 3a",
            ),
            (
                ChainQueryFilterRange::ActionHashTerminated(actions[2].as_hash().clone(), 1),
                vec![false, true, true, false, false, false, false],
                "hash terminated not start",
            ),
            (
                ChainQueryFilterRange::ActionHashTerminated(actions[2].as_hash().clone(), 0),
                vec![false, false, true, false, false, false, false],
                "hash terminated not start 0 prior",
            ),
            (
                ChainQueryFilterRange::ActionHashTerminated(actions[5].as_hash().clone(), 7),
                vec![true, true, true, true, false, true, false],
                "hash terminated main chain before chain start",
            ),
            (
                ChainQueryFilterRange::ActionHashTerminated(actions[4].as_hash().clone(), 7),
                vec![true, true, true, false, true, false, false],
                "hash terminated 3a chain before chain start",
            ),
        ] {
            assert_eq!(
                (
                    map_query(
                        &ChainQueryFilter::new().sequence_range(sequence_range),
                        &actions,
                    ),
                    name
                ),
                (expected, name),
            );
        }
    }

    #[test]
    fn filter_by_multi() {
        let actions = fixtures();

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .action_type(actions[0].action_type())
                    .entry_type(actions[0].entry_type().unwrap().clone())
                    .sequence_range(ChainQueryFilterRange::ActionSeqRange(0, 0)),
                &actions
            ),
            [true, false, false, false, false, false, false].to_vec()
        );

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .action_type(actions[1].action_type())
                    .entry_type(actions[0].entry_type().unwrap().clone())
                    .sequence_range(ChainQueryFilterRange::ActionSeqRange(0, 999)),
                &actions
            ),
            [false, false, false, false, false, true, false].to_vec()
        );

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .entry_type(actions[0].entry_type().unwrap().clone())
                    .sequence_range(ChainQueryFilterRange::ActionSeqRange(0, 999)),
                &actions
            ),
            [true, false, false, false, true, true, false].to_vec()
        );
    }
}
