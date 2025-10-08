use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_sqlite::prelude::{DbKindDht, DbRead};
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::ToSql;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::MUST_GET_AGENT_ACTIVITY;
use holochain_state::prelude::*;
use std::cmp::Reverse;
use std::collections::HashSet;

#[cfg(test)]
mod test;

/// Get the agent activity for a given agent and
/// filtered range of actions.
///
/// If the full filtered range of activity is found, this will return [`MustGetAgentActivityResponse::Activity`].
/// If the chain top is not found, this will return [`MustGetAgentActivityResponse::ChainTopNotFound`].
/// If the chain top is found, but the full range of activity within the filter was not found,
/// this will return [`MustGetAgentActivityResponse::IncompleteChain`].
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn must_get_agent_activity(
    env: DbRead<DbKindDht>,
    author: AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<MustGetAgentActivityResponse> {
    let mut activity = env
        .read_async({
            let filter = filter.clone();
            let author = author.clone();

            move |txn| get_filtered_agent_activity(txn, &author, filter)
        })
        .await?;

    // Remove forked activity from activity list
    exclude_forked_activity(&mut activity);

    let result = 
        // If no activity was returned, then we never found the chain top hash specified by the filter.
        if activity.len() == 0 {
            let chain_top = filter.clone().chain_top;
            MustGetAgentActivityResponse::ChainTopNotFound(chain_top)
        }

        // If activity list does not contain the full sequence of activity
        // from start of filtered range through end, or if the sequence activity
        // is not hash-chained, then it is incomplete.
        else if !is_activity_complete(&activity) || !is_activity_chained_descending(&activity) {
            MustGetAgentActivityResponse::IncompleteChain
        }

        // Otherwise, activity is complete.
        else {
            let warrants = env
                .read_async(move |txn| CascadeTxnWrapper::from(txn).get_warrants_for_agent(&author, true))
                .await?;
            MustGetAgentActivityResponse::Activity {
                activity,
                warrants,
            }
        };

    Ok(result)
}


/// Get the agent activity for a given range of actions from the Scratch.
/// Note that Scratch actions should always be more recently created than database actions
/// and thus will have a higher action seq than any actions in the database.
pub(crate) fn get_filtered_agent_activity_from_scratch(
    scratch: &mut Scratch,
    author: &AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    match scratch.actions().find(|a| a.hashed.hash == filter.chain_top) {
        // If the filter's chain top Action is in scratch space, then we need to get some Actions from scratch.
        // Otherwise, we know there are no Actions in Scratch that are within the filter range.
        Some(chain_top_action) => {
            // If ChainFilter includes until hashes, get the *first* Action that is contained in the until hashes set.
            // The *first* Action will have the highest action seq.
            // TODO: is this accurate?
            let mut max_until_hash_action = None;
            if let Some(until_hashes) = filter.get_until_hash() {
                max_until_hash_action = scratch.actions().find(|a| until_hashes.contains(a.hashed.action_hash()))
            }

            // Filter scratch Actions by ChainFilter
            let activity = scratch
                .actions()
                .filter(|a| {
                    let action = a.action();
                    let is_author = action.author() == author;
                    let is_lte_chain_top = action.action_seq() <= chain_top_action.seq();

                    let mut is_gte_until_timestamp = true;
                    if let Some(until_timestamp) = filter.get_until_timestamp() {
                        is_gte_until_timestamp = a.action().timestamp() >= until_timestamp;
                    }

                    let mut is_gte_max_until_hash_seq = true;
                    if let Some(until_action) = max_until_hash_action {
                        is_gte_max_until_hash_seq = a.action().action_seq() >= until_action.seq();
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
        },
        None => Ok(vec![])
    }
    
}

/// Get the agent activity for a given range of actions from the database.
pub(crate) fn get_filtered_agent_activity(
    txn: &Transaction,
    author: &AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    // Get the max action seq of all Actions in the set of until hashes.
    let chain_filter_limit_conditions_until_hashes_max_seq = if let Some(filter_hashes) = filter.get_until_hash() {
        // Construct sql query with placeholders for list elements
        let filter_hashes_placeholder = filter_hashes.iter().map(|_| "?").collect::<Vec<&str>>().join(", ");
        let sql_query_seq_hash_in_set = format!("
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
        ");

        // Prepare query parameters
        let mut query_params: Vec<Box<dyn ToSql>> = filter_hashes.iter().map(|h| -> Box<dyn ToSql> {
            Box::new(h.clone())
        }).collect();
        query_params.push(Box::new(author));
        query_params.push(Box::new(ChainOpType::RegisterAgentActivity));

        let query_params_refs: Vec<&dyn ToSql> = query_params.iter().map(|v| v.as_ref()).collect();
        let query_params_refs_slice: &[&dyn ToSql] = query_params_refs.as_slice();
        
        // Execute query
        let max_action_seq: Option<u32> = txn
            .prepare(&sql_query_seq_hash_in_set)?
            .query_row(
                query_params_refs_slice,
                |row| row.get(0)
            )?;

        max_action_seq
    } else {
        None
    };

    // Get the agent activity, filtered by the chain top, author, 3 optional lower-bounds, and optional limit size.
    let out = txn
        .prepare(MUST_GET_AGENT_ACTIVITY)?
        .query_and_then(
            named_params! {
                ":author": author,
                ":op_type_register_agent_activity": ChainOpType::RegisterAgentActivity,
                ":chain_filter_chain_top": filter.chain_top,
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

/// Merge, sort by action seq descending, and deduplicate a list of RegisterAgentActivity lists
pub(crate) fn merge_agent_activity(activity_lists: Vec<Vec<RegisterAgentActivity>>) -> Vec<RegisterAgentActivity> {
    flatten_deduplicate_sort(activity_lists, |a| Reverse(a.action.seq()))
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
pub(crate) fn is_activity_complete(activity: &Vec<RegisterAgentActivity>) -> bool {
    let complete_seqs: HashSet<u32> = (activity[activity.len() - 1].action.seq()..=activity[0].action.seq()).collect();
    let found_seqs: HashSet<u32> = activity.iter().map(|a| a.action.seq()).collect();

    found_seqs == complete_seqs
}

/// Check that every Action's prev_hash is equivalent to the next Action in the list's ActionHash.
pub(crate) fn is_activity_chained_descending(activity: &Vec<RegisterAgentActivity>) -> bool {
    activity
        .windows(2)
        .all(|window| {
            let [w1, w2] = window else { return true; };

            w1.action.prev_hash() == Some(&w2.action.hashed.hash)
        })
}
