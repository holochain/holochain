use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::ToSql;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::ACTION_HASH_TO_SEQ;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::MUST_GET_AGENT_ACTIVITY;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::prelude::WarrantOp;
use std::cmp::Reverse;
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

/// Get the max action_seq of ChainFilter UntilHashes.
fn get_chain_filter_limit_conditions_until_hashes_max_seq(
    scratch: &mut Scratch,
    filter: ChainFilter,
) -> Option<u32> {
    // Get the max action seq of all Actions in the set of until hashes.
    if let Some(until_hashes) = filter.get_until_hash() {
        scratch
            .actions()
            .filter(|a| until_hashes.contains(a.hashed.action_hash()))
            .max_by_key(|a| a.seq())
            .map(|a| a.seq())
    } else {
        None
    }
}

/// Get the agent activity for a given range of actions from the Scratch.
/// Note that Scratch actions should always be more recently created than database actions
/// and thus will have a higher action seq than any actions in the database.
pub(crate) fn get_filtered_agent_activity_from_scratch(
    scratch: &mut Scratch,
    author: &AgentPubKey,
    filter: ChainFilter,
    filter_chain_top_action_seq: u32,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    // Get the max action seq of all Actions in the set of until hashes.
    let chain_filter_limit_conditions_until_hashes_max_seq =
        get_chain_filter_limit_conditions_until_hashes_max_seq(scratch, filter.clone());

    // Until hash Action seq must be less than or equal to ChainFilter chain_top Action seq
    if let Some(until_action_seq) = chain_filter_limit_conditions_until_hashes_max_seq {
        if until_action_seq > filter_chain_top_action_seq {
            return Err(StateQueryError::InvalidInput("The largest ChainFilter until hash Action seq must be less than or equal to the ChainFilter chain_top action seq.".to_string()));
        }
    }

    // Get the agent activity, filtered by the chain top, author, 3 optional lower-bounds, and optional limit size.
    let activity = scratch
        .actions()
        .filter(|a| {
            let action = a.action();
            let is_author = action.author() == author;
            let is_lte_chain_top = action.action_seq() <= filter_chain_top_action_seq;

            let mut is_gte_until_timestamp = true;
            if let Some(until_timestamp) = filter.get_until_timestamp() {
                is_gte_until_timestamp = a.action().timestamp() >= until_timestamp;
            }

            let mut is_gte_max_until_hash_seq = true;
            if let Some(until_action) = chain_filter_limit_conditions_until_hashes_max_seq {
                is_gte_max_until_hash_seq = a.action().action_seq() >= until_action;
            }

            is_author && is_lte_chain_top && is_gte_until_timestamp && is_gte_max_until_hash_seq
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
                ChainIntegrityWarrant::ChainFork { .. } => {
                    unimplemented!("Chain fork warrants are not implemented.");
                }
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
    filter: ChainFilter,
    filter_chain_top_action_seq: u32,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    // Get the max action seq of all Actions in the set of until hashes.
    let chain_filter_limit_conditions_until_hashes_max_seq = if let Some(filter_hashes) =
        filter.get_until_hash()
    {
        // Construct sql query with placeholders for list elements.
        //
        // We cannot keep this sql query in a standalone file,
        // because at compile-time we don't know how many '?' placeholders to include.
        let filter_hashes_placeholder = filter_hashes
            .iter()
            .map(|_| "?")
            .collect::<Vec<&str>>()
            .join(", ");
        let sql_query_seq_hash_in_set = format!(
            "
            SELECT
                MAX(Action.seq) as max_action_seq
            FROM
                Action
                JOIN DhtOp ON DhtOp.action_hash = Action.hash
            WHERE
                Action.hash IN ({filter_hashes_placeholder})
                AND Action.author = ?
                AND DhtOp.type = ?
                AND DhtOp.when_integrated IS NOT NULL
        "
        );

        // Prepare query parameters
        let mut query_params: Vec<Box<dyn ToSql>> = filter_hashes
            .iter()
            .map(|h| -> Box<dyn ToSql> { Box::new(h.clone()) })
            .collect();
        query_params.push(Box::new(author));
        query_params.push(Box::new(ChainOpType::RegisterAgentActivity));

        let query_params_refs: Vec<&dyn ToSql> = query_params.iter().map(|v| v.as_ref()).collect();
        let query_params_refs_slice: &[&dyn ToSql] = query_params_refs.as_slice();

        // Execute query
        let max_action_seq: Option<u32> = txn
            .prepare(&sql_query_seq_hash_in_set)?
            .query_row(query_params_refs_slice, |row| row.get("max_action_seq"))?;

        max_action_seq
    } else {
        None
    };

    // Until hash Action seq must be less than or equal to ChainFilter chain_top Action seq
    if let Some(until_action_seq) = chain_filter_limit_conditions_until_hashes_max_seq {
        if until_action_seq > filter_chain_top_action_seq {
            return Err(StateQueryError::InvalidInput("The largest ChainFilter until hash Action seq must be less than or equal to the ChainFilter chain_top action seq.".to_string()));
        }
    }

    // Get the agent activity, filtered by the chain top, author, 3 optional lower-bounds, and optional limit size.
    let out = txn
        .prepare(MUST_GET_AGENT_ACTIVITY)?
        .query_and_then(
            named_params! {
                ":author": author,
                ":op_type_register_agent_activity": ChainOpType::RegisterAgentActivity,
                ":chain_filter_chain_top_action_seq": filter_chain_top_action_seq,
                ":chain_filter_limit_conditions_until_hashes_max_seq": chain_filter_limit_conditions_until_hashes_max_seq,
                ":chain_filter_limit_conditions_until_timestamp": filter.get_until_timestamp(),
                ":chain_filter_limit_conditions_take": filter.get_take(),
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

/// Remove all Actions that have a sequence number equivalent to any prior Actions.
/// If there are multiple forked Actions with the same sequence number, this retains only the first.
pub(crate) fn exclude_forked_activity(activity: &mut Vec<RegisterAgentActivity>) {
    let mut activity_seqs = HashSet::new();
    activity.retain(|a| activity_seqs.insert(a.action.seq()));
}

/// Check that the complete set of Action sequence numbers is included in the RegisterAgentActivity list
/// which must be already sorted by Action sequence number descending.
pub(crate) fn is_activity_complete_descending(activity: &[RegisterAgentActivity]) -> bool {
    // Check if activity is empty
    if activity.is_empty() {
        return true;
    }

    // Get min and max Action seqs
    let max = activity[0].action.seq();
    let min = activity[activity.len() - 1].action.seq();
    if max < min {
        return false;
    }

    // Check that activity length matches action seq range
    if max - min + 1 != activity.len() as u32 {
        return false;
    }

    // Check that activity includes complete action seq range
    activity
        .windows(2)
        .all(|w| w[0].action.seq() == w[1].action.seq() + 1)
}

/// Check that every Action's prev_hash is equivalent to the next Action in the list's ActionHash.
pub(crate) fn is_activity_chained_descending(activity: &[RegisterAgentActivity]) -> bool {
    activity.windows(2).all(|window| {
        let [w1, w2] = window else {
            return true;
        };

        w1.action.prev_hash() == Some(&w2.action.hashed.hash)
    })
}
