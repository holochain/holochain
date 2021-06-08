use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pCellT;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::TryInto;
use tracing::*;

use crate::core::queue_consumer::WorkComplete;

use super::error::WorkflowResult;

#[cfg(test)]
mod tests;

#[instrument(skip(vault, network))]
/// Send validation receipts to their authors in serial and without waiting for
/// responses.
/// TODO: Currently still waiting for responses because we don't have a network call
/// that doesn't.
pub async fn validation_receipt_workflow(
    vault: EnvWrite,
    network: &mut HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    // Get the env and keystore
    let keystore = vault.keystore();
    // Who we are.
    let validator = network.from_agent();

    // Get out all ops that are marked for sending receipt.
    // FIXME: Test this query.
    let receipts = vault.conn()?.with_reader(|txn| {
        let mut stmt = txn.prepare(
            "
            SELECT Header.author, DhtOp.hash, DhtOp.validation_status,
            DhtOp.when_integrated_ns
            From DhtOp
            JOIN Header ON DhtOp.header_hash = Header.hash
            WHERE
            DhtOp.require_receipt = 1
            AND
            DhtOp.when_integrated_ns IS NOT NULL
            AND
            DhtOp.validation_status IS NOT NULL
            ",
        )?;
        let ops = stmt
            .query_and_then([], |r| {
                let author: AgentPubKey = r.get("author")?;
                let dht_op_hash = r.get("hash")?;
                let validation_status = r.get("validation_status")?;
                let when_integrated = from_blob::<Timestamp>(r.get("when_integrated_ns")?)?;
                StateQueryResult::Ok((
                    ValidationReceipt {
                        dht_op_hash,
                        validation_status,
                        validator: validator.clone(),
                        when_integrated,
                    },
                    author,
                ))
            })?
            .collect::<StateQueryResult<Vec<_>>>()?;
        StateQueryResult::Ok(ops)
    })?;

    // Send the validation receipts
    for (receipt, author) in receipts {
        // Don't send receipt to self.
        if author == validator {
            continue;
        }

        let op_hash = receipt.dht_op_hash.clone();

        // Sign on the dotted line.
        let receipt = receipt.sign(&keystore).await?;

        // Send it and don't wait for response.
        // TODO: When networking has a send without response we can use that
        // instead of waiting for response.
        if let Err(e) = network
            .send_validation_receipt(author, receipt.try_into()?)
            .await
        {
            // No one home, they will need to publish again.
            info!(failed_send_receipt = ?e);
        }
        // Attempted to send the receipt so we now mark
        // it to not send in the future.
        vault
            .async_commit(|txn| set_require_receipt(txn, op_hash, false))
            .await?;
    }

    Ok(WorkComplete::Complete)
}
