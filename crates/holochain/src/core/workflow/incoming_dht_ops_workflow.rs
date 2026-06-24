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

// TODO(read-migration): once legacy DhtOp writes are removed, switch
// this intra-transaction dedup to rely on the new DhtStore. For now
// it remains on the legacy txn to preserve same-transaction semantics.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(txn, ops)))]
fn batch_process_entry(
    txn: &mut Txn<DbKindDht>,
    ops: Vec<DhtOpHashed>,
) -> WorkflowResult<Vec<DhtOpHashed>> {
    // add incoming ops to the validation limbo
    let mut to_pending = Vec::with_capacity(ops.len());
    for op in ops {
        if !op_exists_inner(txn, &op.hash)? {
            to_pending.push(op);
        }
    }

    tracing::debug!("Inserting {} ops", to_pending.len());
    // #5370: legacy DhtOp dual-write removed; `add_to_pending` (and the
    // op_exists_inner dedup read above) remain as dead code pending DbKindDht
    // retirement.

    Ok(to_pending)
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
) -> WorkflowResult<()> {
    let Space {
        incoming_op_hashes,
        incoming_ops_batch,
        dht_db,
        dht_store,
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

    // Do not pre-filter against the new store here. The legacy DHT table and
    // the new store deduplicate independently, so an op present in one but
    // missing from the other must still be written to the other; filtering up
    // front against only the new store would let an op that is in the new store
    // but missing from the legacy table be skipped forever, so the legacy
    // mirror could never catch up (and vice versa). Each write path below
    // deduplicates against its own store, so both stores self-heal.
    if filter_ops.is_empty() {
        // TODO(read-migration): restore this trace once dedup-filtering moves
        // onto the new DhtStore (after the legacy DhtOp table is removed). At
        // that point an empty `filter_ops` again means "all ops were already
        // present", which is worth tracing.
        // tracing::trace!(
        //     "Skipping the rest of the incoming_dht_ops_workflow because all ops were filtered out"
        // );
        return Ok(());
    }

    let (mut maybe_batch, rcv) = incoming_ops_batch.check_insert(filter_ops);

    let incoming_ops_batch = incoming_ops_batch.clone();
    if maybe_batch.is_some() {
        // there was no already running batch task, so spawn one:
        tokio::task::spawn({
            let dht_db = dht_db.clone();
            async move {
                while let Some(entries) = maybe_batch {
                    let senders = Arc::new(parking_lot::Mutex::new(Vec::new()));
                    let senders2 = senders.clone();

                    // All ops received in this batch, written to both stores.
                    let batch_ops: Vec<DhtOpHashed> =
                        entries.iter().flat_map(|e| e.ops.iter().cloned()).collect();

                    // Legacy DHT table — still read by the cascade for
                    // dependency resolution. `batch_process_entry` skips ops
                    // already in the table, so this is a safe backfill.
                    let legacy_result = dht_db
                        .write_async(move |txn| {
                            for entry in entries {
                                let InOpBatchEntry { snd, ops } = entry;
                                let res = batch_process_entry(txn, ops);
                                senders2.lock().push((snd, res.map(|_| ())));
                            }

                            WorkflowResult::Ok(())
                        })
                        .await;

                    // New DHT store — read by sys-validation. Skip ops already
                    // present anywhere in the store so integrated ops are not
                    // re-added to limbo, then record the genuinely new ops.
                    // Gate this on the legacy write succeeding: if legacy failed
                    // we leave the ops un-stored so they are redelivered, rather
                    // than recording them only in the new store (which would be a
                    // permanent mirror gap, since gossip would then consider us
                    // to already hold them).
                    let mut recorded_new = false;
                    if let Err(err) = legacy_result {
                        tracing::error!(?err, "incoming_dht_ops_workflow legacy mirror error");
                    } else {
                        match dht_store.as_read().filter_existing_ops(batch_ops).await {
                            Ok(new_ops) if new_ops.is_empty() => {}
                            Ok(new_ops) => match dht_store.record_incoming_ops(new_ops).await {
                                Ok(()) => recorded_new = true,
                                Err(err) => tracing::error!(
                                    ?err,
                                    "incoming_dht_ops_workflow new-DB write error"
                                ),
                            },
                            Err(err) => {
                                tracing::error!(
                                    ?err,
                                    "incoming_dht_ops_workflow new-DB filter error"
                                )
                            }
                        }
                    }

                    for (snd, res) in senders.lock().drain(..) {
                        let _ = snd.send(res);
                    }

                    // sys-validation reads the new store, so only trigger when
                    // genuinely new ops were recorded there.
                    if recorded_new {
                        tracing::debug!(
                            "Incoming dht ops workflow is now triggering the sys_validation_trigger"
                        );
                        sys_validation_trigger.trigger(&"incoming_dht_ops_workflow");
                    }

                    maybe_batch = incoming_ops_batch.check_end();
                }
            }
        });
    }

    rcv.await
        .map_err(|_| super::error::WorkflowError::RecvError)?
}

/// If this op fails the counterfeit check it should be dropped
#[cfg_attr(feature = "instrument", tracing::instrument(skip(op)))]
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

// #5370: dead pending full DbKindDht retirement.
#[allow(dead_code)]
fn add_to_pending(txn: &mut Txn<DbKindDht>, ops: &[DhtOpHashed]) -> StateMutationResult<()> {
    for op in ops {
        insert_op_dht(
            txn,
            op,
            holochain_serialized_bytes::encode(op.as_content())?.len() as u32,
            todo_no_cache_transfer_data(),
        )?;

        // As validators right now, we always try to send
        // validation receipts.
        set_require_receipt(txn, op.as_hash(), true)?;
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
        .read_async(move |txn| op_exists_inner(txn, &hash))
        .await
}
