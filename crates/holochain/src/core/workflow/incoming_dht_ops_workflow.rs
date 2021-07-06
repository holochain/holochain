//! The workflow and queue consumer for DhtOp integration

use super::error::WorkflowResult;
use super::sys_validation_workflow::counterfeit_check;
use crate::core::queue_consumer::TriggerSender;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::dht_op::DhtOp;
use holochain_types::prelude::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::instrument;

#[cfg(test)]
mod test;

type InOpBatchSnd = tokio::sync::oneshot::Sender<WorkflowResult<()>>;
type InOpBatchRcv = tokio::sync::oneshot::Receiver<WorkflowResult<()>>;

struct InOpBatchEntry {
    snd: InOpBatchSnd,
    from_agent: Option<AgentPubKey>,
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
    from_agent: Option<AgentPubKey>,
    request_validation_receipt: bool,
    ops: Vec<(DhtOpHash, DhtOp)>,
) -> (Option<Vec<InOpBatchEntry>>, InOpBatchRcv) {
    let (snd, rcv) = tokio::sync::oneshot::channel();
    let entry = InOpBatchEntry {
        snd,
        from_agent,
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
    from_agent: Option<AgentPubKey>,
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
            if needs_receipt(&op, &from_agent) && request_validation_receipt {
                set_send_receipt(txn, hash.clone())?;
            }
        }
    }

    add_to_pending(txn, to_pending, from_agent, request_validation_receipt)?;

    Ok(())
}

#[instrument(skip(vault, sys_validation_trigger, ops))]
pub async fn incoming_dht_ops_workflow(
    vault: &EnvWrite,
    mut sys_validation_trigger: TriggerSender,
    ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    from_agent: Option<AgentPubKey>,
    request_validation_receipt: bool,
) -> WorkflowResult<()> {
    let mut filter_ops = Vec::new();

    for (hash, op) in ops {
        if should_keep(&op).await? {
            filter_ops.push((hash, op));
        } else {
            tracing::warn!(
                msg = "Dropping op because it failed counterfeit checks",
                ?op
            );
        }
    }

    let kind = vault.kind().clone();
    let (mut maybe_batch, rcv) = batch_check_insert(
        kind.clone(),
        from_agent,
        request_validation_receipt,
        filter_ops,
    );

    let vault = vault.clone();
    if maybe_batch.is_some() {
        // there was no already running batch task, so spawn one:
        tokio::task::spawn(async move {
            while let Some(entries) = maybe_batch {
                match vault
                    .async_commit(move |txn| {
                        let mut senders = Vec::new();

                        for entry in entries {
                            let InOpBatchEntry {
                                snd,
                                from_agent,
                                request_validation_receipt,
                                ops,
                            } = entry;
                            let res = batch_process_entry(
                                txn,
                                from_agent,
                                request_validation_receipt,
                                ops,
                            );
                            // we can't send the results here...
                            // we haven't comitted
                            senders.push((snd, res));
                        }

                        WorkflowResult::Ok(senders)
                    })
                    .await
                {
                    Err(err) => {
                        tracing::error!(?err, "incoming_dht_ops_workflow error");
                    }
                    Ok(senders) => {
                        for (snd, res) in senders {
                            let _ = snd.send(res);
                        }

                        // trigger validation of queued ops
                        sys_validation_trigger.trigger();
                    }
                }

                maybe_batch = batch_check_end(kind.clone());
            }
        });
    }

    rcv.await.expect("sender dropped")
}

fn needs_receipt(op: &DhtOp, from_agent: &Option<AgentPubKey>) -> bool {
    from_agent
        .as_ref()
        .map(|a| a == op.header().author())
        .unwrap_or(false)
}

#[instrument(skip(op))]
/// If this op fails the counterfeit check it should be dropped
async fn should_keep(op: &DhtOp) -> WorkflowResult<bool> {
    let header = op.header();
    let signature = op.signature();
    Ok(counterfeit_check(signature, &header).await?)
}

fn add_to_pending(
    txn: &mut rusqlite::Transaction<'_>,
    ops: Vec<DhtOpHashed>,
    from_agent: Option<AgentPubKey>,
    request_validation_receipt: bool,
) -> StateMutationResult<()> {
    for op in ops {
        let send_receipt = needs_receipt(&op, &from_agent) && request_validation_receipt;
        let op_hash = op.as_hash().clone();
        insert_op(txn, op, false)?;
        set_require_receipt(txn, op_hash, send_receipt)?;
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

pub fn op_exists(vault: &EnvWrite, hash: &DhtOpHash) -> DatabaseResult<bool> {
    vault.conn()?.with_reader(|txn| op_exists_inner(&txn, hash))
}

fn set_send_receipt(
    txn: &mut rusqlite::Transaction<'_>,
    hash: DhtOpHash,
) -> StateMutationResult<()> {
    set_require_receipt(txn, hash, true)?;
    StateMutationResult::Ok(())
}
