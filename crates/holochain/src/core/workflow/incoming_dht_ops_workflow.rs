//! The workflow and queue consumer for DhtOp integration

use super::error::WorkflowResult;
use super::sys_validation_workflow::counterfeit_check;
use crate::core::queue_consumer::TriggerSender;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use tracing::instrument;

#[cfg(test)]
mod test;

#[instrument(skip(vault, sys_validation_trigger, ops))]
pub async fn incoming_dht_ops_workflow(
    vault: &EnvWrite,
    mut sys_validation_trigger: TriggerSender,
    ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    from_agent: Option<AgentPubKey>,
    request_validation_receipt: bool,
) -> WorkflowResult<()> {
    // add incoming ops to the validation limbo
    for (hash, op) in ops {
        if !op_exists(&vault, &hash)? {
            if should_keep(&op).await? {
                let op = DhtOpHashed::from_content_sync(op);
                add_to_pending(&vault, op, from_agent.clone(), request_validation_receipt)?;
            } else {
                tracing::warn!(
                    msg = "Dropping op because it failed counterfeit checks",
                    ?op
                );
            }
        } else {
            // Check if we should set receipt to send.
            if needs_receipt(&op, &from_agent) && request_validation_receipt {
                set_send_receipt(vault, hash.clone())?;
            }
            // Check if it's authored and needs to be integrated
            set_authored_to_pending_integration(vault, hash)?;
        }
    }

    // trigger validation of queued ops
    sys_validation_trigger.trigger();

    Ok(())
}

fn set_authored_to_pending_integration(
    vault: &EnvWrite,
    hash: DhtOpHash,
) -> StateMutationResult<()> {
    let mut conn = vault.conn()?;
    // Avoid taking a write transaction by checking first.
    let is_authored = conn.with_reader(|txn| {
        StateMutationResult::Ok(txn.query_row(
            "
            SELECT EXISTS(
                SELECT 1 FROM DhtOp 
                WHERE hash = :hash
                AND is_authored = 1
                AND when_integrated IS NULL
                AND validation_stage IS NULL
                AND validation_status IS NOT NULL
            )",
            named_params! {
                ":hash": hash,
            },
            |row| row.get(0),
        )?)
    })?;
    if is_authored {
        conn.with_commit(|txn| {
            txn.execute(
                "
                UPDATE DhtOp
                SET
                validation_stage = 3
                WHERE hash = :hash
                AND is_authored = 1
                AND when_integrated IS NULL
                AND validation_stage IS NULL
                AND validation_status IS NOT NULL
                ",
                named_params! {
                    ":hash": hash,
                },
            )?;
            StateMutationResult::Ok(())
        })?;
    }
    Ok(())
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
    env: &EnvWrite,
    op: DhtOpHashed,
    from_agent: Option<AgentPubKey>,
    request_validation_receipt: bool,
) -> StateMutationResult<()> {
    let send_receipt = needs_receipt(&op, &from_agent) && request_validation_receipt;
    tracing::debug!(?op);

    let op_hash = op.as_hash().clone();
    env.conn()?.with_commit(|txn| {
        insert_op(txn, op, false)?;
        set_require_receipt(txn, op_hash, send_receipt)?;
        StateMutationResult::Ok(())
    })?;
    Ok(())
}

pub fn op_exists(vault: &EnvWrite, hash: &DhtOpHash) -> DatabaseResult<bool> {
    let exists = vault.conn()?.with_reader(|txn| {
        let mut stmt = txn.prepare(
            "
            SELECT 
            *
            FROM DhtOp
            WHERE
            DhtOp.hash = :hash
            ",
        )?;
        DatabaseResult::Ok(stmt.exists(named_params! {
            ":hash": hash,
        })?)
    })?;
    Ok(exists)
}

fn set_send_receipt(vault: &EnvWrite, hash: DhtOpHash) -> StateMutationResult<()> {
    vault.conn()?.with_commit(|txn| {
        set_require_receipt(txn, hash, true)?;
        StateMutationResult::Ok(())
    })?;
    Ok(())
}
