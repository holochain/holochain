use futures::future::BoxFuture;
use std::collections::HashSet;
use std::sync::Arc;

use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::TryInto;
use tracing::*;

use super::error::WorkflowResult;
use crate::core::queue_consumer::WorkComplete;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTarget;
use holochain_zome_types::block::CellBlockReason;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod unit_tests;

#[cfg(test)]
mod unit_tests;

pub async fn pending_receipts(
    vault: &DbRead<DbKindDht>,
    validators: Vec<AgentPubKey>,
) -> StateQueryResult<Vec<(ValidationReceipt, AgentPubKey, DhtOpHash)>> {
    vault
        .read_async(move |txn| get_pending_validation_receipts(&txn, validators))
        .await
}

#[instrument(skip(vault, network, keystore, apply_block))]
/// Send validation receipts to their authors in serial and without waiting for
/// responses.
/// TODO: Currently still waiting for responses because we don't have a network call
/// that doesn't.
pub async fn validation_receipt_workflow<B>(
    dna_hash: Arc<DnaHash>,
    vault: DbWrite<DbKindDht>,
    network: impl HolochainP2pDnaT,
    keystore: MetaLairClient,
    running_cell_ids: HashSet<CellId>,
    apply_block: B,
) -> WorkflowResult<WorkComplete>
where
    B: Fn(Block) -> BoxFuture<'static, DatabaseResult<()>>,
{
    if running_cell_ids.is_empty() {
        return Ok(WorkComplete::Complete);
    }

    // This is making an assumption about the behaviour of validation: Once validation has run on this conductor
    // then all the cells running the same DNA agree on the result.
    let validators = running_cell_ids
        .into_iter()
        .filter_map(|id| {
            let (d, a) = id.into_dna_and_agent();
            if d == *dna_hash {
                Some(a)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Get out all ops that are marked for sending receipt.
    let receipts = pending_receipts(&vault, validators.clone()).await?;

    let validators: HashSet<_> = validators.into_iter().collect();

    // Send the validation receipts
    for (receipt, author, _) in &receipts {
        // Don't send receipt to self. Don't block self.
        if validators.contains(author) {
            continue;
        }

        // Block authors of invalid ops.
        if matches!(receipt.validation_status, ValidationStatus::Rejected) {
            // Block BEFORE we integrate the outcome because this is not atomic
            // and if something goes wrong we know the integration will retry.
            apply_block(Block::new(
                BlockTarget::Cell(
                    CellId::new((*dna_hash).clone(), author.clone()),
                    CellBlockReason::InvalidOp(receipt.dht_op_hash.clone()),
                ),
                InclusiveTimestampInterval::try_new(Timestamp::MIN, Timestamp::MAX)?,
            ))
            .await?;
        }

        // Sign on the dotted line.
        let receipt = match ValidationReceipt::sign(receipt.clone(), &keystore).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                return Ok(WorkComplete::Incomplete);
            }
            Err(e) => {
                info!(failed_to_sign_receipt = ?e);
                return Ok(WorkComplete::Incomplete);
            }
        };

        // Send it and don't wait for response.
        // TODO: When networking has a send without response we can use that
        // instead of waiting for response.
        if let Err(e) = holochain_p2p::HolochainP2pDnaT::send_validation_receipt(
            &network,
            author.clone(),
            receipt.try_into()?,
        )
        .await
        {
            // No one home, they will need to publish again.
            info!(failed_send_receipt = ?e);
        }
    }

    // Record that every receipt has been processed. This is after the main loop
    // above so that we pick up self receipts as well.
    for (_, _, dht_op_hash) in receipts {
        // Attempted to send the receipt so we now mark
        // it to not send in the future.
        vault
            .write_async(move |txn| set_require_receipt(txn, &dht_op_hash, false))
            .await?;
    }

    Ok(WorkComplete::Complete)
}
