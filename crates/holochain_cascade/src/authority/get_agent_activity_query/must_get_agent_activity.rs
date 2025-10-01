use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_sqlite::prelude::{DbKindDht, DbRead};
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::*;
use holochain_state::prelude::*;
use std::ops::RangeInclusive;

#[cfg(test)]
mod test;

/// Get the agent activity for a given agent and
/// hash bounded range of actions.
///
/// If the filter produces an empty range, this will return [`MustGetAgentActivityResponse::EmptyRange`].
/// If the chain top is not found, this will return [`MustGetAgentActivityResponse::ChainTopNotFound`].
/// If the chain top is found, but the full range of activity within the filter was not found,
/// this will return [`MustGetAgentActivityResponse::IncompleteChain`].
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn must_get_agent_activity(
    env: DbRead<DbKindDht>,
    author: AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<MustGetAgentActivityResponse> {
    let bounded_response = env
        .read_async(move |txn| get_bounded_activity(txn, None, &author, filter))
        .await?;
    let response: MustGetAgentActivityResponse = bounded_response.into();
    Ok(response)
}

pub fn get_bounded_activity(
    txn: &Transaction,
    scratch: Option<&Scratch>,
    author: &AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<BoundedMustGetAgentActivityResponse> {
    // Find the bounds of the range specified in the filter.
    let txn = CascadeTxnWrapper::from(txn);
    let warrants = txn.get_warrants_for_agent(author, true)?;

    match find_bounds(&txn, scratch, author, filter)? {
        Sequences::Found(filter) => {
            // Get the full range of actions from the database.
            get_activity(&txn, scratch, author, filter.range()).map(|activity| {
                BoundedMustGetAgentActivityResponse::Activity {
                    activity,
                    filter,
                    warrants,
                }
            })
        }
        // One of the actions specified in the filter does not exist in the database.
        Sequences::ChainTopNotFound(a) => {
            Ok(BoundedMustGetAgentActivityResponse::ChainTopNotFound(a))
        }
        // The filter specifies a range that is empty.
        Sequences::EmptyRange => Ok(BoundedMustGetAgentActivityResponse::EmptyRange),
    }
}

/// Find the filter's sequence bounds.
fn find_bounds(
    txn: &Transaction,
    scratch: Option<&Scratch>,
    author: &AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<Sequences> {
    let mut by_hash_statement = txn.prepare(ACTION_HASH_TO_SEQ)?;

    // Map an action hash to its sequence.
    let get_seq_from_hash = |hash: &ActionHash| {
        if let Some(scratch) = scratch {
            if let Some(action) = scratch.actions().find(|a| a.action_address() == hash) {
                return Ok(Some(action.action().action_seq()));
            }
        }
        let result = by_hash_statement
                .query_row(named_params! {":hash": hash, ":author": author, ":activity": ChainOpType::RegisterAgentActivity}, |row| {
                    row.get(0)
                })
                .optional()?;
        Ok(result)
    };

    // Get the oldest action in a given time period.
    let mut by_ts_statement = txn.prepare(TS_TO_SEQ)?;
    let get_seq_from_ts = |until: Timestamp| {
        // Search in DB
        let db_result = by_ts_statement
          .query_row(named_params! {":until": until.as_micros(), ":author": author, ":activity": ChainOpType::RegisterAgentActivity}, |row| {
              row.get(0)
          })
          .optional()?;
        // Search in scratch
        let scratch_result = scratch.and_then(|s| {
            s.actions()
                .filter_map(|a| {
                    if a.get_timestamp() >= until {
                        Some(a.seq())
                    } else {
                        None
                    }
                })
                .min()
        });
        // Return lowest of the two
        Ok([db_result, scratch_result].into_iter().flatten().min())
    };

    // For all the hashes in the filter, get their sequences.
    Sequences::find_sequences(filter, get_seq_from_hash, get_seq_from_ts)
}

/// Get the agent activity for a given range of actions
/// from the database.
fn get_activity(
    txn: &Transaction,
    scratch: Option<&Scratch>,
    author: &AgentPubKey,
    range: &RangeInclusive<u32>,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    let mut out = txn
        .prepare(MUST_GET_AGENT_ACTIVITY)?
        .query_and_then(
            named_params! {
                 ":author": author,
                 ":op_type": ChainOpType::RegisterAgentActivity,
                 ":lower_seq": range.start(),
                 ":upper_seq": range.end(),
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
    if let Some(scratch) = scratch {
        let iter = scratch
            .actions()
            .filter(|a| {
                let action = a.action();
                action.author() == author
                    && action.action_seq() >= *range.start()
                    && action.action_seq() <= *range.end()
            })
            .map(|action| RegisterAgentActivity {
                action: action.clone(),
                // TODO: Implement getting the cached entries.
                cached_entry: None,
            });
        out.extend(iter);
    }
    Ok(out)
}
