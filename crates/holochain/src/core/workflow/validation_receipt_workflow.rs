use futures::future::BoxFuture;
use futures::{stream, StreamExt};
use itertools::Itertools;
use std::collections::HashSet;
use std::sync::Arc;

use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::*;
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

#[instrument(skip(vault, network, keystore, apply_block))]
/// Send validation receipts to their authors in serial and without waiting for responses.
pub async fn validation_receipt_workflow<B>(
    dna_hash: Arc<DnaHash>,
    vault: DbWrite<DbKindDht>,
    network: impl HolochainP2pDnaT,
    keystore: MetaLairClient,
    running_cell_ids: HashSet<CellId>,
    apply_block: B,
) -> WorkflowResult<WorkComplete>
where
    B: Fn(Block) -> BoxFuture<'static, DatabaseResult<()>> + Clone,
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

    let grouped_by_author = receipts
        .into_iter()
        .group_by(|(_, author)| author.clone())
        .into_iter()
        .map(|(author, receipts)| {
            (
                author,
                receipts.into_iter().map(|(r, _)| r).collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<(AgentPubKey, Vec<ValidationReceipt>)>>();

    for (author, receipts) in grouped_by_author {
        // Try to send the validation receipts
        match sign_and_send_receipts_to_author(
            &dna_hash,
            &network,
            &keystore,
            &validators,
            &author,
            receipts.clone(),
            apply_block.clone(),
        )
        .await
        {
            Ok(()) => {
                // Success, move on to mark them as sent
            }
            Err(e) => {
                info!(failed_to_sign_and_send_receipt = ?e);
            }
        }

        // Attempted to send the receipts so we now mark them to not send in the next workflow run.
        for receipt in receipts {
            vault
                .write_async(move |txn| set_require_receipt(txn, &receipt.dht_op_hash, false))
                .await?;
        }
    }

    Ok(WorkComplete::Complete)
}

/// Perform the signing and sending of
/// Requires that the receipts to send are all by the same author.
async fn sign_and_send_receipts_to_author<B>(
    dna_hash: &DnaHash,
    network: &impl HolochainP2pDnaT,
    keystore: &MetaLairClient,
    validators: &HashSet<AgentPubKey>,
    op_author: &AgentPubKey,
    receipts: Vec<ValidationReceipt>,
    apply_block: B,
) -> WorkflowResult<()>
where
    B: Fn(Block) -> BoxFuture<'static, DatabaseResult<()>>,
{
    // Don't send receipt to self. Don't block self.
    if validators.contains(op_author) {
        return Ok(());
    }

    let num_receipts = receipts.len();

    let receipts: Vec<SignedValidationReceipt> = stream::iter(receipts)
        .filter_map(|receipt| async {
            // Block authors of invalid ops.
            if matches!(receipt.validation_status, ValidationStatus::Rejected) {
                // Block BEFORE we integrate the outcome because this is not atomic
                // and if something goes wrong we know the integration will retry.
                if let Err(e) = apply_block(Block::new(
                    BlockTarget::Cell(
                        CellId::new((*dna_hash).clone(), op_author.clone()),
                        CellBlockReason::InvalidOp(receipt.dht_op_hash.clone()),
                    ),
                    match InclusiveTimestampInterval::try_new(Timestamp::MIN, Timestamp::MAX) {
                        Ok(interval) => interval,
                        Err(e) => {
                            error!("Failed to create timestamp interval: {:?}", e);
                            return None;
                        }
                    },
                ))
                .await
                {
                    error!("Failed to apply block to author {:?}: {:?}", op_author, e)
                }
            }

            // Sign on the dotted line.
            match ValidationReceipt::sign(receipt, keystore).await {
                Ok(r) => r,
                Err(e) => {
                    // TODO Which errors are retryable here? A fatal error would keep being retried and we don't want that;
                    //      aggressively give up for now.
                    info!(failed_to_sign_receipt = ?e);
                    None
                }
            }
        })
        .collect()
        .await;

    if receipts.is_empty() {
        info!("Dropped all validation receipts for author {:?}", op_author);
        return Ok(());
    } else if num_receipts < receipts.len() {
        info!(
            "Dropped {}/{} validation receipts for author {:?}, check previous errors to see why",
            num_receipts - receipts.len(),
            num_receipts,
            op_author,
        );
    }

    // Send it and don't wait for response.
    if let Err(e) = holochain_p2p::HolochainP2pDnaT::send_validation_receipts(
        network,
        op_author.clone(),
        receipts.into(),
    )
    .await
    {
        // No one home, they will need to publish again.
        info!(failed_send_receipt = ?e);
    }

    Ok(())
}

async fn pending_receipts(
    vault: &DbRead<DbKindDht>,
    validators: Vec<AgentPubKey>,
) -> StateQueryResult<Vec<(ValidationReceipt, AgentPubKey)>> {
    vault
        .read_async(move |txn| get_pending_validation_receipts(&txn, validators))
        .await
}
