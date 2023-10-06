//! The workflow and queue consumer for DhtOp integration

use super::error::WorkflowResult;
use super::sys_validation_workflow::counterfeit_check;
use crate::{conductor::space::Space, core::queue_consumer::TriggerSender};
use holo_hash::DhtOpHash;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::dht_op::DhtOp;
use holochain_types::prelude::*;
use incoming_ops_batch::InOpBatchEntry;
use std::{collections::HashSet, sync::Arc};
use tracing::instrument;

mod incoming_ops_batch;

pub use incoming_ops_batch::IncomingOpsBatch;

#[cfg(test)]
mod tests;

struct OpsClaim {
    incoming_op_hashes: IncomingOpHashes,
    working_hashes: Vec<DhtOpHash>,
}

impl OpsClaim {
    fn acquire(
        incoming_op_hashes: IncomingOpHashes,
        ops: Vec<(DhtOpHash, DhtOp)>,
    ) -> (Self, Vec<(DhtOpHash, DhtOp)>) {
        let keep_incoming_op_hashes = incoming_op_hashes.clone();

        // Lock the shared state while we claim the ops we're going to work on
        let mut set = incoming_op_hashes.0.lock();

        // Track the hashes that we're going to work on, and should be removed from the shared state
        // when this claim is dropped.
        let mut working_hashes = Vec::with_capacity(ops.len());
        let mut working_ops = Vec::with_capacity(ops.len());

        for (hash, op) in ops {
            if !set.contains(&hash) {
                set.insert(hash.clone());
                working_hashes.push(hash.clone());
                working_ops.push((hash, op));
            }
        }

        (
            Self {
                incoming_op_hashes: keep_incoming_op_hashes,
                working_hashes,
            },
            working_ops,
        )
    }
}

impl Drop for OpsClaim {
    fn drop(&mut self) {
        // Lock the shared state while we remove the ops we're finished working with
        let incoming_op_hashes = self.incoming_op_hashes.clone();
        let mut set = incoming_op_hashes.0.lock();

        for hash in &self.working_hashes {
            set.remove(hash);
        }
    }
}

#[instrument(skip(txn, ops))]
fn batch_process_entry(
    txn: &mut rusqlite::Transaction<'_>,
    request_validation_receipt: bool,
    ops: Vec<(DhtOpHash, DhtOp)>,
) -> WorkflowResult<()> {
    // add incoming ops to the validation limbo
    let mut to_pending = Vec::with_capacity(ops.len());
    for (hash, op) in ops {
        // TODO this has already been checked at this point hasn't it? We've filtered by ops that are already persisted
        //      and have claimed the current set of ops as in-flight...
        if !op_exists_inner(txn, &hash)? {
            let op = DhtOpHashed::from_content_sync(op);
            to_pending.push(op);
        } else {
            // TODO so is it possible to get here? If not that seems quite critical.
            //      That would actually mean that validation receipts on republish is broken I think?
            //      UPDATE: This doesn't get called in any of our tests so we either can't reach this code or don't
            //              have a test for a failed op ingest.
            //      Update again, it is called if request_validation_receipt is set to true because the previous filter will not run...
            // Check if we should set receipt to send.
            if request_validation_receipt {
                set_send_receipt(txn, &hash)?;
            }
        }
    }

    tracing::debug!("Inserting {} ops", to_pending.len());
    add_to_pending(txn, &to_pending, request_validation_receipt)?;

    Ok(())
}

#[derive(Default, Clone)]
pub struct IncomingOpHashes(Arc<parking_lot::Mutex<HashSet<DhtOpHash>>>);

// TODO This can be called concurrently because it's called from the p2p_event_task!
#[instrument(skip(space, sys_validation_trigger, ops))]
pub async fn incoming_dht_ops_workflow(
    space: Space,
    sys_validation_trigger: TriggerSender,
    mut ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    request_validation_receipt: bool,
) -> WorkflowResult<()> {
    let Space {
        incoming_op_hashes,
        incoming_ops_batch,
        dht_db,
        ..
    } = space;

    // Filter out ops that are already being tracked, so we don't do duplicate work
    let (_claim, mut ops) = OpsClaim::acquire(incoming_op_hashes, ops);

    // TODO we empty check here but not after this so we can queue empty and launch a tokio task, should probably clean
    //      that up.
    if ops.is_empty() {
        return Ok(());
    }

    let mut filter_ops = Vec::new();
    if !request_validation_receipt {
        // TODO not safe to exit here, should clear the incoming_op_hashes with hashes_to_remove before leaving
        // Filter the list of ops to only include those that are not already in the database.
        ops = filter_existing_ops(&dht_db, ops).await?;
    }

    for (hash, op) in ops {
        // It's cheaper to check if the op exists before trying
        // to check the signature or open a write transaction.
        match should_keep(&op).await {
            Ok(()) => filter_ops.push((hash, op)),
            Err(e) => {
                tracing::warn!(
                    msg = "Dropping op because it failed counterfeit checks",
                    ?op
                );
                // TODO we are returning here without blocking this author?
                // TODO Returning here means later ops will never be processed, so the log message should
                //      at least try to tell us that all remaining ops passed to this workflow are being dropped.
                return Err(e);
            }
        }
    }

    let (mut maybe_batch, rcv) =
        incoming_ops_batch.check_insert(request_validation_receipt, filter_ops);

    let incoming_ops_batch = incoming_ops_batch.clone();
    if maybe_batch.is_some() {
        // there was no already running batch task, so spawn one:
        tokio::task::spawn({
            let dht_db = dht_db.clone();
            async move {
                while let Some(entries) = maybe_batch {
                    let senders = Arc::new(parking_lot::Mutex::new(Vec::new()));
                    let senders2 = senders.clone();
                    if let Err(err) = dht_db
                        .write_async(move |txn| {
                            for entry in entries {
                                let InOpBatchEntry {
                                    snd,
                                    request_validation_receipt,
                                    ops,
                                } = entry;
                                let res = batch_process_entry(txn, request_validation_receipt, ops);

                                // we can't send the results here...
                                // we haven't committed
                                senders2.lock().push((snd, res));
                            }

                            WorkflowResult::Ok(())
                        })
                        .await
                    {
                        tracing::error!(?err, "incoming_dht_ops_workflow error");
                    }

                    for (snd, res) in senders.lock().drain(..) {
                        let _ = snd.send(res);
                    }

                    // trigger validation of queued ops
                    tracing::info!(
                        "Incoming dht ops workflow is now triggering the sys_validation_trigger"
                    );
                    sys_validation_trigger.trigger(&"incoming_dht_ops_workflow");

                    maybe_batch = incoming_ops_batch.check_end();
                }
            }
        });
    }

    let r = rcv
        .await
        .map_err(|_| super::error::WorkflowError::RecvError)?;

    r
}

#[instrument(skip(op))]
/// If this op fails the counterfeit check it should be dropped
async fn should_keep(op: &DhtOp) -> WorkflowResult<()> {
    let action = op.action();
    let signature = op.signature();
    Ok(counterfeit_check(signature, &action).await?)
}

fn add_to_pending(
    txn: &mut rusqlite::Transaction<'_>,
    ops: &[DhtOpHashed],
    request_validation_receipt: bool,
) -> StateMutationResult<()> {
    for op in ops {
        insert_op(txn, op)?;
        set_require_receipt(txn, op.as_hash(), request_validation_receipt)?;
    }
    StateMutationResult::Ok(())
}

fn op_exists_inner(txn: &rusqlite::Transaction<'_>, hash: &DhtOpHash) -> DatabaseResult<bool> {
    DatabaseResult::Ok(txn.query_row(
        "
        SELECT EXISTS(
            SELECT
            1
            FROM DhtOp
            WHERE
            DhtOp.hash = :hash
        )
        ",
        named_params! {
            ":hash": hash,
        },
        |row| row.get(0),
    )?)
}

pub async fn op_exists(vault: &DbWrite<DbKindDht>, hash: DhtOpHash) -> DatabaseResult<bool> {
    vault
        .read_async(move |txn| op_exists_inner(&txn, &hash))
        .await
}

pub async fn filter_existing_ops(
    vault: &DbWrite<DbKindDht>,
    mut ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
) -> DatabaseResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>> {
    vault
        .read_async(move |txn| {
            ops.retain(|(hash, _)| !op_exists_inner(&txn, hash).unwrap_or(true));
            Ok(ops)
        })
        .await
}

fn set_send_receipt(
    txn: &mut rusqlite::Transaction<'_>,
    hash: &DhtOpHash,
) -> StateMutationResult<()> {
    set_require_receipt(txn, hash, true)?;
    StateMutationResult::Ok(())
}
