//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::queue_consumer::WorkComplete;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::sql::sql_cell::*;
use holochain_state::prelude::*;
use kitsune2_api::StoredOp;
use rusqlite::Params;

#[cfg(test)]
mod tests;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(vault, network, dht_query_cache))
)]
pub async fn integrate_dht_ops_workflow(
    vault: DbWrite<DbKindDht>,
    dht_query_cache: DhtDbQueryCache,
    network: HolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let time = holochain_zome_types::prelude::Timestamp::now();
    // Get any activity from the cache that is ready to be integrated.
    let activity_to_integrate = dht_query_cache.get_activity_to_integrate().await?;
    let (activity_integrated, stored_ops) = vault
        .write_async(move |txn| {
            let mut stored_ops = Vec::new();
            if !activity_to_integrate.is_empty() {
                let mut stmt = txn.prepare_cached(UPDATE_INTEGRATE_DEP_ACTIVITY)?;
                for (author, seq_range) in &activity_to_integrate {
                    let start = seq_range.start();
                    let end = seq_range.end();
                    let integrated_ops = stmt.query_map(
                        named_params! {
                            ":when_integrated": time,
                            ":register_activity": ChainOpType::RegisterAgentActivity,
                            ":seq_start": start,
                            ":seq_end": end,
                            ":author": author,
                        },
                        to_stored_op,
                    )?;
                    for integrated_op in integrated_ops {
                        stored_ops.push(integrated_op?);
                    }
                }
            }
            // Set all kinds of op types to integrated in DHT database and add op hashes to
            // list of stored ops.
            set_ops_to_integrated(
                txn,
                UPDATE_INTEGRATE_STORE_RECORD,
                named_params! {
                    ":when_integrated": time,
                    ":store_record": ChainOpType::StoreRecord,
                },
                &mut stored_ops,
            )?;

            set_ops_to_integrated(
                txn,
                UPDATE_INTEGRATE_STORE_ENTRY,
                named_params! {
                    ":when_integrated": time,
                    ":store_entry": ChainOpType::StoreEntry,
                },
                &mut stored_ops,
            )?;

            set_ops_to_integrated(
                txn,
                UPDATE_INTEGRATE_DEP_STORE_ENTRY,
                named_params! {
                    ":when_integrated": time,
                    ":updated_content": ChainOpType::RegisterUpdatedContent,
                    ":deleted_entry_action": ChainOpType::RegisterDeletedEntryAction,
                    ":store_entry": ChainOpType::StoreEntry,
                },
                &mut stored_ops,
            )?;

            set_ops_to_integrated(
                txn,
                SET_ADD_LINK_OPS_TO_INTEGRATED,
                named_params! {
                    ":when_integrated": time,
                    ":create_link": ChainOpType::RegisterAddLink,
                },
                &mut stored_ops,
            )?;

            set_ops_to_integrated(
                txn,
                UPDATE_INTEGRATE_DEP_STORE_RECORD,
                named_params! {
                    ":when_integrated": time,
                    ":store_record": ChainOpType::StoreRecord,
                    ":updated_record": ChainOpType::RegisterUpdatedRecord,
                    ":deleted_by": ChainOpType::RegisterDeletedBy,
                },
                &mut stored_ops,
            )?;

            set_ops_to_integrated(
                txn,
                SET_DELETE_LINK_OPS_TO_INTEGRATED,
                named_params! {
                    ":when_integrated": time,
                    ":create_link": ChainOpType::RegisterAddLink,
                    ":delete_link": ChainOpType::RegisterRemoveLink,

                },
                &mut stored_ops,
            )?;

            set_ops_to_integrated(
                txn,
                SET_CHAIN_INTEGRITY_WARRANT_OPS_TO_INTEGRATED,
                named_params! {
                    ":when_integrated": time,
                    ":chain_integrity_warrant": WarrantOpType::ChainIntegrityWarrant,
                },
                &mut stored_ops,
            )?;

            WorkflowResult::Ok((activity_to_integrate, stored_ops))
        })
        .await?;
    // Once the database transaction is committed, update the cache with the
    // integrated activity.
    dht_query_cache
        .set_all_activity_to_integrated(activity_integrated)
        .await?;
    let changed = stored_ops.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps);
    if changed > 0 {
        network.new_integrated_data(stored_ops).await?;
        Ok(WorkComplete::Incomplete(None))
    } else {
        Ok(WorkComplete::Complete)
    }
}

fn to_stored_op(row: &Row<'_>) -> rusqlite::Result<StoredOp> {
    let op_hash = row.get::<_, DhtOpHash>(0)?;
    let created_at = row.get::<_, Timestamp>(1)?;
    let op = StoredOp {
        created_at: kitsune2_api::Timestamp::from_micros(created_at.as_micros()),
        op_id: op_hash.to_k2_op(),
    };
    Ok(op)
}

fn set_ops_to_integrated(
    txn: &mut Txn<'_, '_, DbKindDht>,
    sql: &str,
    params: impl Params,
    stored_ops: &mut Vec<StoredOp>,
) -> WorkflowResult<()> {
    let mut stmt = txn.prepare(sql)?;
    let integrated_ops = stmt.query_map(params, to_stored_op)?;
    for op_hash in integrated_ops {
        stored_ops.push(op_hash?);
    }
    Ok(())
}
