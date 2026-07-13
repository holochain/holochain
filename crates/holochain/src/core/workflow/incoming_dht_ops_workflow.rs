//! The workflow and queue consumer for DhtOp integration

use super::sys_validation_workflow::counterfeit_check_action;
use super::{error::WorkflowResult, sys_validation_workflow::counterfeit_check_warrant};
use crate::{conductor::space::Space, core::queue_consumer::TriggerSender};
use holo_hash::DhtOpHash;
use holochain_types::dht_v2::{DhtOp, DhtOpHashed};
use incoming_ops_batch::InOpBatchEntry;
use std::{collections::HashSet, sync::Arc};

mod incoming_ops_batch;

pub use incoming_ops_batch::IncomingOpsBatch;

#[cfg(test)]
mod tests;

/// An incoming DHT op paired with a flag to request a validation receipt.
///
/// Published ops request validation receipts, other ops like gossiped ones
/// do not.
#[derive(Debug, Clone)]
pub struct IncomingDhtOp {
    pub op: DhtOpHashed,
    pub require_validation_receipt: bool,
}

struct OpsClaim {
    incoming_op_hashes: IncomingOpHashes,
    working_hashes: Vec<DhtOpHash>,
}

impl OpsClaim {
    fn acquire(
        incoming_op_hashes: IncomingOpHashes,
        ops: Vec<IncomingDhtOp>,
    ) -> (Self, Vec<IncomingDhtOp>) {
        let keep_incoming_op_hashes = incoming_op_hashes.clone();

        // Lock the shared state while we claim the ops we're going to work on
        let mut set = incoming_op_hashes.0.lock();

        // Track the hashes that we're going to work on, and should be removed from the shared state
        // when this claim is dropped.
        let mut working_hashes = Vec::with_capacity(ops.len());
        let mut working_ops = Vec::with_capacity(ops.len());

        for op in ops {
            if !set.contains(&op.op.hash) {
                set.insert(op.op.hash.clone());
                working_hashes.push(op.op.hash.clone());
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

#[derive(Default, Clone)]
pub struct IncomingOpHashes(Arc<parking_lot::Mutex<HashSet<DhtOpHash>>>);

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(space, sys_validation_trigger, ops))
)]
pub async fn incoming_dht_ops_workflow(
    space: Space,
    sys_validation_trigger: TriggerSender,
    ops: Vec<(DhtOp, bool)>,
) -> WorkflowResult<()> {
    let Space {
        incoming_op_hashes,
        incoming_ops_batch,
        dht_store,
        ..
    } = space;

    // Convert DhtOps to IncomingOps by computing hashes
    let ops = ops
        .into_iter()
        .map(|(op, require_validation_receipt)| IncomingDhtOp {
            op: DhtOpHashed::from_content_sync(op),
            require_validation_receipt,
        })
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
        let keeper = should_keep(&op.op.content).await;
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

    let (mut maybe_batch, rcv) = incoming_ops_batch.check_insert(filter_ops);

    let incoming_ops_batch = incoming_ops_batch.clone();
    if maybe_batch.is_some() {
        // there was no already running batch task, so spawn one:
        tokio::task::spawn(async move {
            while let Some(entries) = maybe_batch {
                // Collect the ops from this batch, paired with the flag to
                // request a validation receipt, keeping the senders to notify
                // once the batch is stored.
                let mut senders = Vec::with_capacity(entries.len());
                let mut batch_ops: Vec<(DhtOpHashed, bool)> = Vec::new();
                for entry in entries {
                    let InOpBatchEntry { snd, ops } = entry;
                    batch_ops.extend(
                        ops.into_iter()
                            .map(|op| (op.op, op.require_validation_receipt)),
                    );
                    senders.push(snd);
                }

                // Skip ops already present anywhere in the store so integrated
                // ops are not re-added to limbo, then record the genuinely new
                // ops. On failure the ops are left un-stored so they are
                // redelivered.
                let mut recorded_new = false;
                let result: WorkflowResult<()> =
                    match dht_store.as_read().filter_existing_ops(batch_ops).await {
                        Ok(new_ops) if new_ops.is_empty() => Ok(()),
                        Ok(new_ops) => match dht_store.record_incoming_ops(new_ops).await {
                            Ok(()) => {
                                recorded_new = true;
                                Ok(())
                            }
                            Err(err) => {
                                tracing::error!(?err, "incoming_dht_ops_workflow write error");
                                Err(err.into())
                            }
                        },
                        Err(err) => {
                            tracing::error!(?err, "incoming_dht_ops_workflow filter error");
                            Err(err.into())
                        }
                    };

                for snd in senders {
                    let _ = snd.send(match &result {
                        Ok(()) => Ok(()),
                        Err(err) => Err(super::error::WorkflowError::other(err.to_string())),
                    });
                }

                // sys-validation reads the store, so only trigger when
                // genuinely new ops were recorded there.
                if recorded_new {
                    tracing::debug!(
                        "Incoming dht ops workflow is now triggering the sys_validation_trigger"
                    );
                    sys_validation_trigger.trigger(&"incoming_dht_ops_workflow");
                }

                maybe_batch = incoming_ops_batch.check_end();
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
            let signed_action = op.signed_action();
            counterfeit_check_action(signed_action.signature(), signed_action.data()).await?;
        }
        DhtOp::WarrantOp(op) => counterfeit_check_warrant(op).await?,
    }
    Ok(())
}
