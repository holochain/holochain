//! The workflow and queue consumer for DhtOp integration

use super::sys_validation_workflow::counterfeit_check_action;
use super::{error::WorkflowResult, sys_validation_workflow::counterfeit_check_warrant};
use crate::{conductor::space::Space, core::queue_consumer::TriggerSender};
use holo_hash::DhtOpHash;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use incoming_ops_batch::InOpBatchEntry;
use std::{collections::HashSet, sync::Arc};

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
        ops: Vec<DhtOpHashed>,
    ) -> (Self, Vec<DhtOpHashed>) {
        let keep_incoming_op_hashes = incoming_op_hashes.clone();

        // Lock the shared state while we claim the ops we're going to work on
        let mut set = incoming_op_hashes.0.lock();

        // Track the hashes that we're going to work on, and should be removed from the shared state
        // when this claim is dropped.
        let mut working_hashes = Vec::with_capacity(ops.len());
        let mut working_ops = Vec::with_capacity(ops.len());

        for op in ops {
            if !set.contains(&op.hash) {
                set.insert(op.hash.clone());
                working_hashes.push(op.hash.clone());
                working_ops.push(op);
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

#[cfg_attr(feature = "instrument", tracing::instrument(skip(txn, ops)))]
fn batch_process_entry(
    txn: &mut rusqlite::Transaction<'_>,
    request_validation_receipt: bool,
    ops: Vec<DhtOpHashed>,
) -> WorkflowResult<()> {
    // add incoming ops to the validation limbo
    let mut to_pending = Vec::with_capacity(ops.len());
    for op in ops {
        if !op_exists_inner(txn, &op.hash)? {
            to_pending.push(op);
        } else if request_validation_receipt {
            set_require_receipt(txn, &op.hash, true)?;
        }
    }

    tracing::debug!("Inserting {} ops", to_pending.len());
    add_to_pending(txn, &to_pending, request_validation_receipt)?;

    Ok(())
}

#[derive(Default, Clone)]
pub struct IncomingOpHashes(Arc<parking_lot::Mutex<HashSet<DhtOpHash>>>);

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(space, sys_validation_trigger, ops))
)]
pub async fn incoming_dht_ops_workflow(
    space: Space,
    sys_validation_trigger: TriggerSender,
    ops: Vec<DhtOp>,
    request_validation_receipt: bool,
) -> WorkflowResult<()> {
    let Space {
        incoming_op_hashes,
        incoming_ops_batch,
        dht_db,
        ..
    } = space;

    // Compute hashes for all the ops
    let ops = ops
        .into_iter()
        .map(DhtOpHashed::from_content_sync)
        .collect();

    // Filter out ops that are already being tracked, to avoid doing duplicate work
    let (_claim, ops) = OpsClaim::acquire(incoming_op_hashes, ops);

    // If everything we've been sent is already being worked on then this workflow run can be skipped
    if ops.is_empty() {
        return Ok(());
    }

    let num_ops = ops.len();
    let mut filter_ops = Vec::with_capacity(num_ops);
    for op in ops {
        // It's cheaper to check if the signature is valid before proceeding to open a write transaction.
        let keeper = should_keep(&op.content).await;
        match keeper {
            Ok(()) => filter_ops.push(op),
            Err(e) => {
                tracing::warn!(
                    ?op,
                    "Dropping batch of {} ops because the current op failed counterfeit checks",
                    num_ops,
                );
                // TODO we are returning here without blocking this author?
                return Err(e);
            }
        }
    }

    if !request_validation_receipt {
        // Filter the list of ops to only include those that are not already in the database.
        filter_ops = filter_existing_ops(&dht_db, filter_ops).await?;
    }

    // Check again whether everything has been filtered out and avoid launching a Tokio task if so
    if filter_ops.is_empty() {
        tracing::trace!(
            "Skipping the rest of the incoming_dht_ops_workflow because all ops were filtered out"
        );
        return Ok(());
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
                    tracing::debug!(
                        "Incoming dht ops workflow is now triggering the sys_validation_trigger"
                    );
                    sys_validation_trigger.trigger(&"incoming_dht_ops_workflow");

                    maybe_batch = incoming_ops_batch.check_end();
                }
            }
        });
    }

    rcv.await
        .map_err(|_| super::error::WorkflowError::RecvError)?
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip(op)))]
/// If this op fails the counterfeit check it should be dropped
async fn should_keep(op: &DhtOp) -> WorkflowResult<()> {
    match op {
        DhtOp::ChainOp(op) => {
            let action = op.action();
            let signature = op.signature();
            counterfeit_check_action(signature, &action).await?;
        }
        DhtOp::WarrantOp(op) => counterfeit_check_warrant(op).await?,
    }
    Ok(())
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

    Ok(())
}

fn op_exists_inner(txn: &rusqlite::Transaction<'_>, hash: &DhtOpHash) -> DatabaseResult<bool> {
    Ok(txn.query_row(
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
    mut ops: Vec<DhtOpHashed>,
) -> DatabaseResult<Vec<DhtOpHashed>> {
    vault
        .read_async(move |txn| {
            ops.retain(|op| !op_exists_inner(&txn, &op.hash).unwrap_or(true));
            Ok(ops)
        })
        .await
}
