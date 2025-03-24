//! # Publish Dht Op Workflow
//!
//! ## Open questions
//! - [x] Publish add and remove links on private entries, what are the constraints on when to publish
//!
//! For now, Publish links on private entries
// TODO: B-01827 Make story about: later consider adding a flag to make a link private and not publish it.
//       Even for those private links, we may need to publish them to the author of the private entry
//       (and we'd have to reference its action  which actually exists on the DHT to make that work,
//       rather than the entry which does not exist on the DHT).
//!
//!

use super::error::WorkflowResult;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holo_hash::*;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time;
use std::time::Duration;
use tracing::*;

mod publish_query;
pub use publish_query::{get_ops_to_publish, num_still_needing_publish};

#[cfg(test)]
mod unit_tests;

/// Default redundancy factor for validation receipts
pub const DEFAULT_RECEIPT_BUNDLE_SIZE: u8 = 5;

/// Map of required validation receipts.
pub type RequiredReceiptCounts = HashMap<ZomeIndex, HashMap<EntryDefIndex, u8>>;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(db, network, trigger_self, min_publish_interval))
)]
pub async fn publish_dht_ops_workflow(
    authored_db: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    required_receipt_counts: Arc<RequiredReceiptCounts>,
    network: Arc<impl HolochainP2pDnaT>,
    trigger_self: TriggerSender,
    agent: AgentPubKey,
    min_publish_interval: Duration,
) -> WorkflowResult<WorkComplete> {
    let mut complete = WorkComplete::Complete;
    let to_publish = publish_dht_ops_workflow_inner(
        authored_db.clone().into(),
        agent.clone(),
        min_publish_interval,
    )
    .await?;
    let to_publish_count: usize = to_publish.values().map(Vec::len).sum();

    if to_publish_count > 0 {
        info!("publishing {} ops", to_publish_count);
    }

    // Commit to the network
    let mut success = Vec::with_capacity(to_publish.len());
    for (basis, list) in to_publish {
        let (mut op_hash_list, op_data_list): (Vec<_>, Vec<_>) = list.into_iter().unzip();

        // build an exclude list of agents from whom we already have receipts
        let agent2 = agent.clone();
        let op_hash_list2 = op_hash_list.clone();
        let exclude = authored_db
            .read_async(move |txn| {
                let mut exclude = std::collections::HashSet::new();

                // always add ourselves to the exclude list
                exclude.insert(agent2);

                let mut stmt =
                    txn.prepare("SELECT blob FROM ValidationReceipt WHERE op_hash = ?")?;

                for op_hash in op_hash_list2 {
                    for receipt in stmt.query_map([op_hash], |row| {
                        Ok(from_blob::<SignedValidationReceipt>(row.get("blob")?))
                    })? {
                        let receipt = match receipt {
                            Ok(Ok(r)) => r,
                            _ => continue,
                        };

                        for validator in receipt.receipt.validators {
                            exclude.insert(validator);
                        }
                    }
                }

                StateQueryResult::Ok(exclude.into_iter().collect())
            })
            .await?;

        // First, check to see if we can get the required validation receipts.
        match network
            .get_validation_receipts(
                basis.clone(),
                op_hash_list.clone(),
                exclude,
                DEFAULT_RECEIPT_BUNDLE_SIZE as usize,
            )
            .await
        {
            Err(err) => debug!(?err, "error fetching validation receipts"),
            Ok(bundle) => {
                for receipt in bundle.into_iter() {
                    // Get the action for this op so we can check the entry type.
                    let hash = receipt.receipt.dht_op_hash.clone();
                    let action: Option<SignedAction> = authored_db
                        .read_async(move |txn| {
                            let h: Option<Vec<u8>> = txn
                                .query_row(
                                    "
                                    SELECT Action.blob as action_blob
                                    FROM DhtOp
                                    JOIN Action ON Action.hash = DhtOp.action_hash
                                    WHERE DhtOp.hash = :hash
                                    ",
                                    named_params! {
                                        ":hash": hash,
                                    },
                                    |row| row.get("action_blob"),
                                )
                                .optional()?;
                            match h {
                                Some(h) => from_blob(h),
                                None => Ok(None),
                            }
                        })
                        .await?;

                    // If the action has an app entry type get the entry def
                    // from the conductor.
                    let required_receipt_count = match action.as_ref().and_then(|h| h.entry_type())
                    {
                        Some(EntryType::App(AppEntryDef {
                            zome_index,
                            entry_index,
                            ..
                        })) => required_receipt_counts
                            .get(zome_index)
                            .and_then(|z| z.get(entry_index)),
                        _ => None,
                    };

                    // If no required receipt count was found then fallback to the default.
                    let required_validation_count = required_receipt_count.unwrap_or(
                        &crate::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE,
                    );

                    let op_hash = receipt.receipt.dht_op_hash.clone();

                    match process_validation_receipt(
                        authored_db.clone(),
                        dht_db.clone(),
                        receipt,
                        *required_validation_count,
                    )
                    .await
                    {
                        Err(err) => {
                            debug!(?err, "error processing validation receipt");
                        }
                        Ok(false) => (),
                        Ok(true) => {
                            op_hash_list.retain(|e| e != &op_hash);
                            success.push(op_hash);
                        }
                    }
                }
            }
        }

        if op_hash_list.is_empty() {
            // We have enough receipts!
            // Short-circuit so we don't publish.
            continue;
        }

        // second, if we still need validation receipts,
        // try re-publishing the op hashes.
        match network
            .publish(
                true,
                false,
                basis,
                agent.clone(),
                op_hash_list.clone(),
                None,
                Some(op_data_list),
            )
            .await
        {
            Err(e) => {
                // If we get a routing error it means the space hasn't started yet and we should try publishing again.
                if let holochain_p2p::HolochainP2pError::RoutingDnaError(_) = e {
                    // TODO if this doesn't change what is the loop terminate condition?
                    complete = WorkComplete::Incomplete(None);
                }
                warn!(failed_to_send_publish = ?e);
            }
            Ok(()) => {
                success.extend(op_hash_list);
            }
        }
    }

    if to_publish_count > 0 {
        info!("published {}/{} ops", success.len(), to_publish_count);
    }

    let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?;
    let continue_publish = authored_db
        .write_async(move |txn| {
            for hash in success {
                set_last_publish_time(txn, &hash, now)?;
            }
            WorkflowResult::Ok(publish_query::num_still_needing_publish(txn, agent)? > 0)
        })
        .await?;

    // If we have more ops that could be published then continue looping.
    if continue_publish {
        trigger_self.resume_loop();
    } else {
        trigger_self.pause_loop();
    }

    debug!("committed published ops");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    Ok(complete)
}

/// Read the authored for ops with receipt count < R
pub async fn publish_dht_ops_workflow_inner(
    authored_db: DbRead<DbKindAuthored>,
    agent: AgentPubKey,
    min_publish_interval: Duration,
) -> WorkflowResult<HashMap<OpBasis, Vec<(DhtOpHash, crate::prelude::DhtOp)>>> {
    // Ops to publish by basis
    let mut to_publish = HashMap::new();

    for (basis, op_hash, op) in
        get_ops_to_publish(agent, &authored_db, min_publish_interval).await?
    {
        // For every op publish a request
        // Collect and sort ops by basis
        to_publish
            .entry(basis)
            .or_insert_with(Vec::new)
            .push((op_hash, op));
    }

    Ok(to_publish)
}

async fn process_validation_receipt(
    authored_db: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    receipt: SignedValidationReceipt,
    required_validation_count: u8,
) -> WorkflowResult<bool> {
    debug!(from = ?receipt.receipt.validators, hash = ?receipt.receipt.dht_op_hash);

    let receipt_op_hash = receipt.receipt.dht_op_hash.clone();

    let receipt_count = dht_db
        .write_async({
            let receipt_op_hash = receipt_op_hash.clone();
            move |txn| -> StateMutationResult<usize> {
                // Add the new receipts to the db
                add_if_unique(txn, receipt)?;

                // Get the current count for this DhtOp.
                let receipt_count: usize = txn.query_row(
                    "SELECT COUNT(rowid) FROM ValidationReceipt WHERE op_hash = :op_hash",
                    named_params! {
                        ":op_hash": receipt_op_hash,
                    },
                    |row| row.get(0),
                )?;

                if receipt_count >= required_validation_count as usize {
                    // If we have enough receipts then set receipts to complete.
                    //
                    // Don't fail here if this doesn't work, it's only informational. Getting
                    // the same flag set in the authored db is what will stop the publish
                    // workflow from republishing this op.
                    set_receipts_complete_redundantly_in_dht_db(txn, &receipt_op_hash, true).ok();
                }

                Ok(receipt_count)
            }
        })
        .await?;

    // If we have enough receipts then set receipts to complete.
    if receipt_count >= required_validation_count as usize {
        // Note that the flag is set in the authored db because that's what the publish workflow checks to decide
        // whether to republish the op for more validation receipts.
        authored_db
            .write_async(move |txn| -> StateMutationResult<()> {
                set_receipts_complete(txn, &receipt_op_hash, true)
            })
            .await?;

        Ok(true)
    } else {
        Ok(false)
    }
}
