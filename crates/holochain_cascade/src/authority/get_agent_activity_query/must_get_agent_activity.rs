use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::ACTION_HASH_TO_SEQ_AND_TIMESTAMP;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::MUST_GET_AGENT_ACTIVITY;
use holochain_state::prelude::*;
use holochain_types::prelude::WarrantOp;
use holochain_zome_types::prelude::Timestamp;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::collections::HashSet;

#[cfg(test)]
mod test;

/// Get the Action sequence and timestamp for a given hash from a database.
pub(crate) fn get_action_seq_and_timestamp(
    txn: &Transaction,
    author: &AgentPubKey,
    action_hash: &ActionHash,
) -> StateQueryResult<Option<(u32, Timestamp)>> {
    let maybe_action_data: Option<(u32, i64)> = txn
        .prepare(ACTION_HASH_TO_SEQ_AND_TIMESTAMP)?
        .query_row(
            named_params! {
                ":author": author,
                ":action_hash": action_hash,
                ":op_type_register_agent_activity": ChainOpType::RegisterAgentActivity,
            },
            |row| {
                let seq: u32 = row.get("seq")?;
                let timestamp_micros: i64 = row.get("timestamp")?;
                Ok((seq, timestamp_micros))
            },
        )
        .optional()?;

    Ok(maybe_action_data
        .map(|(seq, timestamp_micros)| (seq, Timestamp::from_micros(timestamp_micros))))
}

/// Get the Action sequence and timestamp for a given hash from Scratch.
pub(crate) fn get_action_seq_and_timestamp_from_scratch(
    scratch: &mut Scratch,
    author: &AgentPubKey,
    action_hash: &ActionHash,
) -> StateQueryResult<Option<(u32, Timestamp)>> {
    match scratch
        .actions()
        .find(|a| a.action().author() == author && &a.hashed.hash == action_hash)
    {
        Some(chain_top_action) => {
            let seq = chain_top_action.seq();
            let timestamp = chain_top_action.action().timestamp();
            Ok(Some((seq, timestamp)))
        }
        None => Ok(None),
    }
}

/// Get the agent activity from Scratch, filtered by the chain top, author, and optional until_hash lower-bound.
pub(crate) fn get_filtered_agent_activity_from_scratch(
    scratch: &mut Scratch,
    author: &AgentPubKey,
    filter_chain_top_action_seq: u32,
    resolved_until_action_seq: Option<u32>,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    let activity = scratch
        .actions()
        .filter(|a| {
            let action = a.action();
            let is_author = action.author() == author;
            let is_lte_chain_top = action.action_seq() <= filter_chain_top_action_seq;

            let mut is_gte_max_until_hash_seq = true;
            if let Some(until_action) = resolved_until_action_seq {
                is_gte_max_until_hash_seq = a.action().action_seq() >= until_action;
            }

            is_author && is_lte_chain_top && is_gte_max_until_hash_seq
        })
        .map(|action| RegisterAgentActivity {
            action: action.clone(),
            // TODO: Implement getting the cached entries.
            cached_entry: None,
        })
        .collect();

    Ok(activity)
}

/// Get all warrants against an Agent from scratch
pub(crate) fn get_warrants_for_agent_from_scratch(
    scratch: &mut Scratch,
    agent: &AgentPubKey,
) -> StateQueryResult<Vec<WarrantOp>> {
    let warrants: Vec<WarrantOp> = scratch
        .warrants()
        .filter(|a| {
            let WarrantProof::ChainIntegrity(warrant) = a.proof.clone();

            match warrant {
                ChainIntegrityWarrant::InvalidChainOp { action_author, .. } => {
                    &action_author == agent
                }
                ChainIntegrityWarrant::ChainFork { chain_author, .. } => &chain_author == agent,
            }
        })
        .map(|s| WarrantOp::from(s.clone()))
        .collect();

    Ok(warrants)
}

/// Get the agent activity, filtered by the chain top, author, and optional until_hash lower-bound.
pub(crate) fn get_filtered_agent_activity(
    txn: &Transaction,
    author: &AgentPubKey,
    filter_chain_top_action_seq: u32,
    resolved_until_action_seq: Option<u32>,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    let out = txn
        .prepare(MUST_GET_AGENT_ACTIVITY)?
        .query_and_then(
            named_params! {
                ":author": author,
                ":op_type_register_agent_activity": ChainOpType::RegisterAgentActivity,
                ":chain_filter_chain_top_action_seq": filter_chain_top_action_seq,
                ":chain_filter_limit_conditions_until_hashes_max_seq": resolved_until_action_seq,
            },
            |row| {
                let action: SignedAction = from_blob(row.get("blob")?)?;
                let (action, signature) = action.into();
                let hash: ActionHash = row.get("hash")?;
                let hashed = ActionHashed::with_pre_hashed(action, hash);
                let action = SignedActionHashed::with_presigned(hashed, signature);
                StateQueryResult::Ok(RegisterAgentActivity {
                    action,
                    // TODO: Implement getting the cached entries.
                    cached_entry: None,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(out)
}

/// Flattens, sorts and deduplicates a list of lists.
fn flatten_deduplicate_sort<T, K, F>(lists: Vec<Vec<T>>, mut key_by: F) -> Vec<T>
where
    K: Ord,
    F: FnMut(&T) -> K,
{
    // Flatten list of lists into a single list
    let merged_size = lists.iter().map(|l| l.len()).sum();
    let mut merged = Vec::with_capacity(merged_size);
    for list in lists {
        merged.extend(list);
    }

    // Sort in-place
    merged.sort_unstable_by_key(&mut key_by);

    // Deduplicate in-place
    let dedup_key_by = |x: &mut T| key_by(&*x);
    merged.dedup_by_key(dedup_key_by);

    merged
}

/// Merge, sort by action seq descending, then action hash descending, and deduplicate a list of RegisterAgentActivity lists
pub(crate) fn merge_agent_activity(
    activity_lists: Vec<Vec<RegisterAgentActivity>>,
) -> Vec<RegisterAgentActivity> {
    flatten_deduplicate_sort(activity_lists, |a| {
        (
            Reverse(a.action.seq()),
            Reverse(a.action.hashed.hash.clone()),
        )
    })
}

/// Merge, sort by action seq descending, and deduplicate a list of WarrantOp lists
pub(crate) fn merge_warrants(warrants_lists: Vec<Vec<WarrantOp>>) -> Vec<WarrantOp> {
    flatten_deduplicate_sort(warrants_lists, |w| w.to_hash())
}

/// Remove forked Actions by walking the chain from the provided chain top.
///
/// The input `activity` must already be sorted by action seq descending.
pub(crate) fn exclude_forked_activity(
    activity: &mut Vec<RegisterAgentActivity>,
    chain_top: &ActionHash,
) {
    if activity.is_empty() {
        return;
    }

    let chain_hashes = collect_canonical_chain_hashes(activity, chain_top);
    activity.retain(|a| chain_hashes.contains(&a.action.hashed.hash));
}

/// Walk the chain from `chain_top` backwards, collecting hashes.
///
/// The input `activity` must already be sorted by action seq descending.
/// The returned set contains a chain of hashes reachable from `chain_top`.
fn collect_canonical_chain_hashes(
    activity: &[RegisterAgentActivity],
    chain_top: &ActionHash,
) -> HashSet<ActionHash> {
    // Map each action hash to its position in the descending list
    let index_by_hash: HashMap<ActionHash, usize> = activity
        .iter()
        .enumerate()
        .map(|(i, a)| (a.action.hashed.hash.clone(), i))
        .collect();

    let mut chain_hashes: HashSet<ActionHash> = HashSet::new();

    // Start from chain_top
    let Some(&walk_index) = index_by_hash.get(chain_top) else {
        return chain_hashes;
    };

    // Walk backwards through the chain
    let mut walk_index = walk_index;
    for _ in 0..activity.len() {
        let current = &activity[walk_index];
        let current_hash = current.action.hashed.hash.clone();

        // Stop if we've already visited this hash
        if !chain_hashes.insert(current_hash) {
            break;
        }

        // Move to the previous action in the chain
        let Some(prev_hash) = current.action.prev_hash() else {
            break;
        };

        // Look up the predecessor, stop if it's not in the list
        let Some(&prev_index) = index_by_hash.get(prev_hash) else {
            break;
        };

        walk_index = prev_index;
    }

    chain_hashes
}

/// Apply the `until_timestamp` filter to the activity list
///
/// Returns `true` when at least one action in the activity list
/// has a timestamp less than the `until_timestamp`.
pub(crate) fn apply_timestamp_filter(
    activity: &mut Vec<RegisterAgentActivity>,
    until_timestamp: Option<Timestamp>,
) -> bool {
    match until_timestamp {
        None => false,
        Some(until_ts) => {
            // Check the lowest action *before* trimming.
            let precedes_boundary = activity
                .last()
                .map(|a| a.action.action().timestamp() < until_ts)
                .unwrap_or(false);

            activity.retain(|a| a.action.action().timestamp() >= until_ts);

            precedes_boundary
        }
    }
}

/// An intermediary type to specify the completeness of a set of actions
/// retrieved from must_get_agent_activity queries.
pub(crate) enum MustGetAgentActivityCompleteness {
    Complete,
    IncompleteChain,
    UntilHashMissing(ActionHash),
    UntilTimestampIndeterminate(Timestamp),
}

/// Evaluate whether activity is a complete response by the requested chain filter.
pub(crate) fn check_agent_activity_completeness(
    activity: &[RegisterAgentActivity],
    filter: &ChainFilter,
    canonical_chain_precedes_until_timestamp: bool,
) -> MustGetAgentActivityCompleteness {
    let has_gap = activity
        .windows(2)
        .any(|w| w[0].action.seq() != w[1].action.seq() + 1);
    let reaches_genesis = activity
        .last()
        .map(|last| last.action.seq() == 0)
        .unwrap_or(false);

    match &filter.limit_conditions {
        holochain_zome_types::chain::LimitConditions::ToGenesis => {
            if has_gap || !reaches_genesis {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
        holochain_zome_types::chain::LimitConditions::UntilHash(until_hash) => {
            if !activity.iter().any(|a| &a.action.hashed.hash == until_hash) {
                MustGetAgentActivityCompleteness::UntilHashMissing(until_hash.clone())
            } else if has_gap {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
        holochain_zome_types::chain::LimitConditions::UntilTimestamp(until_timestamp) => {
            let any_satisfies_timestamp = activity
                .iter()
                .any(|a| a.action.action().timestamp() >= *until_timestamp);

            // For the UntilTimestamp response to be deterministic, we must have retreived an action
            // with a timestamp that is *less than* the until timestamp, OR have retreived actions until genesis.
            //
            // This guarantees that we have the complete set of actions greater than or equal to the until timestamp.
            if !any_satisfies_timestamp
                || (!reaches_genesis && !canonical_chain_precedes_until_timestamp)
            {
                MustGetAgentActivityCompleteness::UntilTimestampIndeterminate(*until_timestamp)
            } else if has_gap {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
        holochain_zome_types::chain::LimitConditions::Take(take) => {
            let take = *take as usize;
            if activity.len() >= take {
                if has_gap {
                    MustGetAgentActivityCompleteness::IncompleteChain
                } else {
                    MustGetAgentActivityCompleteness::Complete
                }
            } else if has_gap || !reaches_genesis {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
    }
}
