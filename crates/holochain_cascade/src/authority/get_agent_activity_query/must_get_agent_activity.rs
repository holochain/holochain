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
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::ActionHashed;
use holochain_zome_types::ChainFilter;
use holochain_zome_types::RegisterAgentActivityOp;
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
        .async_reader(move |mut txn| {
            let (range, hashes) = match find_bounds(&mut txn, &author, &filter)? {
                Some(r) => r,
                None => return Ok(None),
            };
            let constraints = ChainFilterConstraints::new(filter, hashes, range.clone());
            get_activity(&mut txn, &author, &range).map(|a| Some((a, constraints)))
        })
        .await?;
    match result {
        Some((activity, constraints)) => Ok(constraints.filter_then_check(activity)),
        None => Ok(MustGetAgentActivityResponse::PositionNotFound),
    }
}

fn hash_to_seq(
    statement: &mut Statement,
    hash: &ActionHash,
    author: &AgentPubKey,
) -> StateQueryResult<Option<u32>> {
    Ok(statement
        .query_row(named_params! {"hash": hash, "author": author}, |row| {
            row.get(0)
        })
        .optional()?)
}

fn find_bounds(
    txn: &mut Transaction,
    author: &AgentPubKey,
    filter: &ChainFilter,
) -> StateQueryResult<Option<(RangeInclusive<u32>, HashMap<ActionHash, u32>)>> {
    let mut statement = txn.prepare(ACTION_HASH_TO_SEQ)?;
    let upper_seq = match hash_to_seq(&mut statement, &filter.position, author)? {
        Some(u) => u,
        None => return Ok(None),
    };
    let hashes = match filter.get_until() {
        Some(hashes) => {
            let hashes = hashes
                .iter()
                .filter_map(|hash| match hash_to_seq(&mut statement, hash, author) {
                    Ok(seq) => Some(Ok((hash.clone(), seq?))),
                    Err(e) => Some(Err(e)),
                })
                .collect::<Result<HashMap<_, _>, _>>()?;
            Some(hashes)
        }
        None => None,
    };
    let lower_seq: u32 = hashes
        .as_ref()
        .and_then(|h| h.values().min().copied())
        .unwrap_or_default();
    Ok(Some((lower_seq..=upper_seq, hashes.unwrap_or_default())))
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
) -> StateQueryResult<Vec<RegisterAgentActivityOp>> {
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
                Ok(RegisterAgentActivityOp { action })
            },
        )?
        .collect::<Result<Vec<_>, _>>()
}
