//! The workflow and queue consumer for DhtOp integration

use super::error::WorkflowResult;
use super::sys_validation_workflow::counterfeit_check;
use crate::core::queue_consumer::TriggerSender;
use holo_hash::DhtOpHash;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::dht_op::DhtOp;
use holochain_types::prelude::*;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

#[cfg(test)]
mod test;

type InOpBatchSnd = tokio::sync::oneshot::Sender<WorkflowResult<()>>;
type InOpBatchRcv = tokio::sync::oneshot::Receiver<WorkflowResult<()>>;

struct InOpBatchEntry {
    snd: InOpBatchSnd,
    request_validation_receipt: bool,
    ops: Vec<(DhtOpHash, DhtOp)>,
}

struct InOpBatch {
    is_running: bool,
    pending: Vec<InOpBatchEntry>,
}

impl Default for InOpBatch {
    fn default() -> Self {
        Self {
            is_running: false,
            pending: Vec::new(),
        }
    }
}

static IN_OP_BATCH: Lazy<parking_lot::Mutex<HashMap<DbKind, InOpBatch>>> =
    Lazy::new(|| parking_lot::Mutex::new(HashMap::new()));

/// if result.0.is_none() -- we queued it to send later
/// if result.0.is_some() -- the batch should be run now
fn batch_check_insert(
    kind: DbKind,
    request_validation_receipt: bool,
    ops: Vec<(DhtOpHash, DhtOp)>,
) -> (Option<Vec<InOpBatchEntry>>, InOpBatchRcv) {
    let (snd, rcv) = tokio::sync::oneshot::channel();
    let entry = InOpBatchEntry {
        snd,
        request_validation_receipt,
        ops,
    };
    let mut lock = IN_OP_BATCH.lock();
    let batch = lock.entry(kind).or_insert_with(InOpBatch::default);
    if batch.is_running {
        // there is already a batch running, just queue this
        batch.pending.push(entry);
        (None, rcv)
    } else {
        // no batch running, run this (and assert we never collect straglers
        assert!(batch.pending.is_empty());
        batch.is_running = true;
        (Some(vec![entry]), rcv)
    }
}

/// if result.is_none() -- we are done, end the loop for now
/// if result.is_some() -- we got more items to process
fn batch_check_end(kind: DbKind) -> Option<Vec<InOpBatchEntry>> {
    let mut lock = IN_OP_BATCH.lock();
    let batch = lock.entry(kind).or_insert_with(InOpBatch::default);
    assert!(batch.is_running);
    let out: Vec<InOpBatchEntry> = batch.pending.drain(..).collect();
    if out.is_empty() {
        // pending was empty, we can end the loop for now
        batch.is_running = false;
        None
    } else {
        // we have some more pending, continue the running loop
        Some(out)
    }
}

fn batch_process_entry(
    txn: &mut rusqlite::Transaction<'_>,
    request_validation_receipt: bool,
    ops: Vec<(DhtOpHash, DhtOp)>,
) -> WorkflowResult<()> {
    // add incoming ops to the validation limbo
    let mut to_pending = Vec::with_capacity(ops.len());
    for (hash, op) in ops {
        if !op_exists_inner(txn, &hash)? {
            let op = DhtOpHashed::from_content_sync(op);
            to_pending.push(op);
        } else {
            // Check if we should set receipt to send.
            if request_validation_receipt {
                set_send_receipt(txn, hash.clone())?;
            }
        }
    }

    add_to_pending(txn, to_pending, request_validation_receipt)?;

    Ok(())
}

#[derive(Default, Clone)]
pub struct IncomingOpHashes(Arc<parking_lot::Mutex<HashSet<DhtOpHash>>>);

#[instrument(skip(vault, sys_validation_trigger, ops, incoming_op_hashes))]
pub async fn incoming_dht_ops_workflow(
    vault: &DbWrite<DbKindDht>,
    incoming_op_hashes: Option<&IncomingOpHashes>,
    sys_validation_trigger: TriggerSender,
    mut ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    request_validation_receipt: bool,
) -> WorkflowResult<()> {
    let mut filter_ops = Vec::new();
    let mut hashes_to_remove = if incoming_op_hashes.is_some() {
        Vec::with_capacity(ops.len())
    } else {
        Vec::with_capacity(0)
    };

    // Filter out ops that are already being tracked, so we don't do duplicate work
    if let Some(incoming_op_hashes) = &incoming_op_hashes {
        let mut set = incoming_op_hashes.0.lock();
        let mut o = Vec::with_capacity(ops.len());
        for (hash, op) in ops {
            if !set.contains(&hash) {
                set.insert(hash.clone());
                hashes_to_remove.push(hash.clone());
                o.push((hash, op));
            }
        }
        ops = o;
    }

    if ops.is_empty() {
        return Ok(());
    }

    for (hash, op) in ops {
        // It's cheaper to check if the op exists before trying
        // to check the signature or open a write transaction.
        if !op_exists(vault, &hash)? {
            match should_keep(&op).await {
                Ok(()) => filter_ops.push((hash, op)),
                Err(e) => {
                    tracing::warn!(
                        msg = "Dropping op because it failed counterfeit checks",
                        ?op
                    );
                    return Err(e);
                }
            }
        }
    }

    let kind = vault.kind().kind().clone();
    let (mut maybe_batch, rcv) =
        batch_check_insert(kind.clone(), request_validation_receipt, filter_ops);

    let vault = vault.clone();
    if maybe_batch.is_some() {
        // there was no already running batch task, so spawn one:
        tokio::task::spawn(async move {
            while let Some(entries) = maybe_batch {
                let senders = Arc::new(parking_lot::Mutex::new(Vec::new()));
                let senders2 = senders.clone();
                if let Err(err) = vault
                    .async_commit(move |txn| {
                        for entry in entries {
                            let InOpBatchEntry {
                                snd,
                                request_validation_receipt,
                                ops,
                            } = entry;
                            let res = batch_process_entry(txn, request_validation_receipt, ops);

                            // we can't send the results here...
                            // we haven't comitted
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
                sys_validation_trigger.trigger();

                maybe_batch = batch_check_end(kind.clone());
            }
        });
    }

    let r = rcv
        .await
        .map_err(|_| super::error::WorkflowError::RecvError)?;

    if let Some(incoming_op_hashes) = &incoming_op_hashes {
        let mut set = incoming_op_hashes.0.lock();
        for hash in hashes_to_remove {
            set.remove(&hash);
        }
    }
    r
}

#[instrument(skip(op))]
/// If this op fails the counterfeit check it should be dropped
async fn should_keep(op: &DhtOp) -> WorkflowResult<()> {
    let header = op.header();
    let signature = op.signature();
    Ok(counterfeit_check(signature, &header).await?)
}

fn add_to_pending(
    txn: &mut rusqlite::Transaction<'_>,
    ops: Vec<DhtOpHashed>,
    request_validation_receipt: bool,
) -> StateMutationResult<()> {
    for op in ops {
        let op_hash = op.as_hash().clone();
        insert_op(txn, op)?;
        set_require_receipt(txn, op_hash, request_validation_receipt)?;
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

pub fn op_exists(vault: &DbWrite<DbKindDht>, hash: &DhtOpHash) -> DatabaseResult<bool> {
    vault.conn()?.with_reader(|txn| op_exists_inner(&txn, hash))
}

fn set_send_receipt(
    txn: &mut rusqlite::Transaction<'_>,
    hash: DhtOpHash,
) -> StateMutationResult<()> {
    set_require_receipt(txn, hash, true)?;
    StateMutationResult::Ok(())
}
