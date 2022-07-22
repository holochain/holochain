use std::collections::HashMap;
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
use holochain_types::chain::ChainFilterConstraints;
use holochain_types::chain::ChainFilterIter;
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

pub async fn must_get_agent_activity(
    env: DbRead<DbKindDht>,
    author: AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<MustGetAgentActivityResponse> {
    let result = env
        .async_reader(
            move |mut txn| match find_bounds(&mut txn, &author, filter)? {
                Sequences::Found(filter_range) => {
                    get_activity(&mut txn, &author, filter_range.range()).map(|a| {
                        ((
                            MustGetAgentActivityResponse::Activity(a),
                            Some(filter_range),
                        ))
                    })
                }
                Sequences::ActionNotFound(a) => {
                    Ok((MustGetAgentActivityResponse::ActionNotFound(a), None))
                }
                Sequences::PositionNotHighest => {
                    Ok((MustGetAgentActivityResponse::PositionNotHighest, None))
                }
                Sequences::EmptyRange => Ok((MustGetAgentActivityResponse::EmptyRange, None)),
            },
        )
        .await?;
    match result {
        (MustGetAgentActivityResponse::Activity(activity), Some(filter_range)) => {
            Ok(filter_range.filter_then_check(activity))
        }
        (MustGetAgentActivityResponse::Activity(_), None) => unreachable!(),
        (r, _) => Ok(r),
    }
}

fn hash_to_seq(
    statement: &mut Statement,
    hash: &ActionHash,
    author: &AgentPubKey,
) -> StateQueryResult<Option<u32>> {
    Ok(statement
        .query_row(named_params! {"hash": hash, "author": author, "activity": DhtOpType::RegisterAgentActivity}, |row| {
            row.get(0)
        })
        .optional()?)
}

fn find_bounds(
    txn: &mut Transaction,
    author: &AgentPubKey,
    filter: ChainFilter,
) -> StateQueryResult<Sequences> {
    let mut statement = txn.prepare(ACTION_HASH_TO_SEQ)?;

    let get_seq = move |hash: &ActionHash| hash_to_seq(&mut statement, hash, author);
    Ok(Sequences::find_sequences(filter, get_seq)?)
}

// fn unique_seq_count(
//     txn: &mut Transaction,
//     author: &AgentPubKey,
//     range: &RangeInclusive<u32>,
// ) -> StateQueryResult<u32> {
//     Ok(txn.prepare(MUST_GET_AGENT_ACTIVITY_COUNT)?.query_row(
//         named_params! {
//                  ":author": author,
//                  ":op_type": DhtOpType::RegisterAgentActivity,
//                  ":lower_seq": range.start(),
//                  ":upper_seq": range.end(),
//         },
//         |row| row.get("unique_seq"),
//     )?)
// }

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
