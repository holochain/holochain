use std::ops::RangeInclusive;

use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::db::DbRead;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::must_get_agent_activity::*;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::StateQueryResult;
use holochain_types::chain::MustGetAgentActivityResponse;
use holochain_types::chain::Sequences;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::ActionHashed;
use holochain_zome_types::ChainFilter;
use holochain_zome_types::RegisterAgentActivity;
use holochain_zome_types::SignedAction;
use holochain_zome_types::SignedActionHashed;

#[cfg(test)]
mod test;

/// Get the agent activity for a given agent and
/// hash bounded range of actions.
///
/// The full range must exist or this will return [`MustGetAgentActivityResponse::IncompleteChain`].
pub async fn must_get_agent_activity(
    env: DbRead<DbKindDht>,
    author: AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<MustGetAgentActivityResponse> {
    let result = env
        .async_reader(
            // Find the bounds of the range specified in the filter.
            move |mut txn| match find_bounds(&mut txn, &author, filter)? {
                Sequences::Found(filter_range) => {
                    // Get the full range of actions from the database.
                    get_activity(&mut txn, &author, filter_range.range()).map(|a| {
                        (
                            MustGetAgentActivityResponse::Activity(a),
                            Some(filter_range),
                        )
                    })
                }
                // One of the actions specified in the filter does not exist in the database.
                Sequences::ActionNotFound(a) => {
                    Ok((MustGetAgentActivityResponse::ActionNotFound(a), None))
                }
                // The filter specifies starting position that has a lower
                // action sequence than another action in the filter.
                Sequences::PositionNotHighest => {
                    Ok((MustGetAgentActivityResponse::PositionNotHighest, None))
                }
                // The filter specifies a range that is empty.
                Sequences::EmptyRange => Ok((MustGetAgentActivityResponse::EmptyRange, None)),
            },
        )
        .await?;
    match result {
        (MustGetAgentActivityResponse::Activity(activity), Some(filter_range)) => {
            // Filter the activity from the database and check the invariants of the
            // filter still hold.
            Ok(filter_range.filter_then_check(activity))
        }
        (MustGetAgentActivityResponse::Activity(_), None) => unreachable!(),
        (r, _) => Ok(r),
    }
}

/// Get the action sequence for a given action hash.
fn hash_to_seq(
    statement: &mut Statement,
    hash: &ActionHash,
    author: &AgentPubKey,
) -> StateQueryResult<Option<u32>> {
    Ok(statement
        .query_row(named_params! {":hash": hash, ":author": author, ":activity": DhtOpType::RegisterAgentActivity}, |row| {
            row.get(0)
        })
        .optional()?)
}

/// Find the filters sequence bounds.
fn find_bounds(
    txn: &mut Transaction,
    author: &AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<Sequences> {
    let mut statement = txn.prepare(ACTION_HASH_TO_SEQ)?;

    // Map an action hash to its sequence.
    let get_seq = move |hash: &ActionHash| hash_to_seq(&mut statement, hash, author);

    // For all the hashes in the filter, get their sequences.
    Ok(Sequences::find_sequences(filter, get_seq)?)
}

/// Get the agent activity for a given range of actions
/// from the database.
fn get_activity(
    txn: &mut Transaction,
    author: &AgentPubKey,
    range: &RangeInclusive<u32>,
) -> StateQueryResult<Vec<RegisterAgentActivity>> {
    txn.prepare(MUST_GET_AGENT_ACTIVITY)?
        .query_and_then(
            named_params! {
                 ":author": author,
                 ":op_type": DhtOpType::RegisterAgentActivity,
                 ":lower_seq": range.start(),
                 ":upper_seq": range.end(),

            },
            |row| {
                let SignedAction(action, signature) = from_blob(row.get("blob")?)?;
                let hash: ActionHash = row.get("hash")?;
                let hashed = ActionHashed::with_pre_hashed(action, hash);
                let action = SignedActionHashed::with_presigned(hashed, signature);
                Ok(RegisterAgentActivity { action })
            },
        )?
        .collect::<Result<Vec<_>, _>>()
}
