use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::ACTION_HASH_TO_SEQ;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::MUST_GET_AGENT_ACTIVITY;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::prelude::WarrantOp;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::collections::HashSet;

#[cfg(test)]
mod test;

/// Get the ChainFilter chain_top Action seq from a database
/// If not found, returns Ok(None)
pub(crate) fn get_action_seq(
    txn: &Transaction,
    author: &AgentPubKey,
    action_hash: &ActionHash,
) -> StateQueryResult<Option<u32>> {
    let maybe_chain_top_action_seq: Option<u32> = txn
        .prepare(ACTION_HASH_TO_SEQ)?
        .query_row(
            named_params! {
                ":author": author,
                ":action_hash": action_hash,
                ":op_type_register_agent_activity": ChainOpType::RegisterAgentActivity,
            },
            |row| row.get::<&str, u32>("seq"),
        )
        .optional()?;

    Ok(maybe_chain_top_action_seq)
}

/// Get the ChainFilter chain_top Action seq from Scratch
/// If not found, returns Ok(None)
pub(crate) fn get_action_seq_from_scratch(
    scratch: &mut Scratch,
    author: &AgentPubKey,
    action_hash: &ActionHash,
) -> StateQueryResult<Option<u32>> {
    match scratch
        .actions()
        .find(|a| a.action().author() == author && &a.hashed.hash == action_hash)
    {
        Some(chain_top_action) => Ok(Some(chain_top_action.seq())),
        None => Ok(None),
    }
}

/// Get the agent activity for a given range of actions from the Scratch.
/// Note that Scratch actions should always be more recently created than database actions
/// and thus will have a higher action seq than any actions in the database.
pub(crate) fn get_filtered_agent_activity_from_scratch(
    scratch: &mut Scratch,
    author: &AgentPubKey,
    filter_chain_top_action_seq: u32,
    resolved_until_action_seq: Option<u32>,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    // Until hash Action seq must be less than or equal to ChainFilter chain_top Action seq
    if let Some(until_action_seq) = resolved_until_action_seq {
        if until_action_seq > filter_chain_top_action_seq {
            return Err(StateQueryError::InvalidInput("The largest ChainFilter until hash Action seq must be less than or equal to the ChainFilter chain_top action seq.".to_string()));
        }
    }

    // Get the agent activity filtered by chain top, author, and optional until_hash lower-bound.
    // The until_timestamp lower-bound is applied in-memory after fork exclusion (same as the
    // database path) so that the canonical_chain_precedes_until_timestamp check sees the full canonical chain.
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

/// Get the agent activity for a given range of actions from the database.
pub(crate) fn get_filtered_agent_activity(
    txn: &Transaction,
    author: &AgentPubKey,
    filter_chain_top_action_seq: u32,
    resolved_until_action_seq: Option<u32>,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    // Until hash Action seq must be less than or equal to ChainFilter chain_top Action seq
    if let Some(until_action_seq) = resolved_until_action_seq {
        if until_action_seq > filter_chain_top_action_seq {
            return Err(StateQueryError::InvalidInput("The largest ChainFilter until hash Action seq must be less than or equal to the ChainFilter chain_top action seq.".to_string()));
        }
    }

    // Get the agent activity, filtered by the chain top, author, and optional until_hash lower-bound.
    // The until_timestamp lower-bound is applied in-memory after fork exclusion so that the
    // canonical_chain_precedes_until_timestamp check can inspect the full canonical chain (including actions below the boundary).
    // We cannot apply the take limit here because a single db result may be missing some
    // sequences that another db has, so the limit must be re-applied after merge.
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
/// The input must be sorted by action seq descending.
pub(crate) fn exclude_forked_activity(
    activity: &mut Vec<RegisterAgentActivity>,
    chain_top: &ActionHash,
) {
    // Short-circuit if there is no activity to filter.
    if activity.is_empty() {
        return;
    }

    // Build a lookup map from action hash to its index in the descending list.
    let index_by_hash: HashMap<ActionHash, usize> = activity
        .iter()
        .enumerate()
        .map(|(i, a)| (a.action.hashed.hash.clone(), i))
        .collect();

    // Track which action hashes belong to the chosen linear chain.
    let mut keep_hashes: HashSet<ActionHash> = HashSet::new();
    // Start from the provided chain top.
    let Some(&start_index) = index_by_hash.get(chain_top) else {
        activity.clear();
        return;
    };
    let mut current_index = start_index;
    // Walk at most the length of the list to avoid infinite loops.
    for _ in 0..activity.len() {
        // Select the current action and its hash.
        let current = &activity[current_index];
        let current_hash = current.action.hashed.hash.clone();
        // Stop if we have already visited this hash (cycle or duplicate).
        if !keep_hashes.insert(current_hash) {
            break;
        }

        // Move to the previous action in the chain if it exists.
        let Some(prev_hash) = current.action.prev_hash() else {
            break;
        };

        // Look up the index of the previous action; stop if it is missing.
        let Some(&next_index) = index_by_hash.get(prev_hash) else {
            break;
        };

        // Continue the walk from the previous action.
        current_index = next_index;
    }

    // Retain only the actions that are part of the selected chain.
    activity.retain(|a| keep_hashes.contains(&a.action.hashed.hash));
}

pub(crate) enum MustGetCompleteness {
    Complete,
    IncompleteChain,
    UntilHashMissing(ActionHash),
    UntilTimestampIndeterminate(Timestamp),
}

/// Evaluate whether activity satisfies the requested chain filter.
pub(crate) fn check_agent_activity_completeness(
    activity: &[RegisterAgentActivity],
    filter: &ChainFilter,
    canonical_chain_precedes_until_timestamp: bool,
) -> MustGetCompleteness {
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
                MustGetCompleteness::IncompleteChain
            } else {
                MustGetCompleteness::Complete
            }
        }
        holochain_zome_types::chain::LimitConditions::UntilHash(until_hash) => {
            if !activity.iter().any(|a| &a.action.hashed.hash == until_hash) {
                MustGetCompleteness::UntilHashMissing(until_hash.clone())
            } else if has_gap {
                MustGetCompleteness::IncompleteChain
            } else {
                MustGetCompleteness::Complete
            }
        }
        holochain_zome_types::chain::LimitConditions::UntilTimestamp(until_timestamp) => {
            if !activity
                .iter()
                .any(|a| a.action.action().timestamp() >= *until_timestamp)
            {
                return MustGetCompleteness::UntilTimestampIndeterminate(*until_timestamp);
            }

            // Deterministic completeness for until_timestamp requires certainty that
            // the returned set contains all actions >= until_timestamp.
            // We have that certainty iff either:
            // - the returned chain reaches genesis, or
            // - the local stores contain at least one action below the timestamp
            //   boundary (for this author and chain range).
            if !reaches_genesis && !canonical_chain_precedes_until_timestamp {
                MustGetCompleteness::UntilTimestampIndeterminate(*until_timestamp)
            } else if has_gap {
                MustGetCompleteness::IncompleteChain
            } else {
                MustGetCompleteness::Complete
            }
        }
        holochain_zome_types::chain::LimitConditions::Take(take) => {
            let take = *take as usize;
            if activity.len() >= take {
                MustGetCompleteness::Complete
            } else if has_gap || !reaches_genesis {
                MustGetCompleteness::IncompleteChain
            } else {
                MustGetCompleteness::Complete
            }
        }
    }
}
