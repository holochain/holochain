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

enum SendOutcome {
    Attempted,
    AuthorUnavailable,
}

pub async fn pending_receipts(
    vault: &DbRead<DbKindDht>,
    validators: Vec<AgentPubKey>,
) -> StateQueryResult<Vec<(ValidationReceipt, AgentPubKey, DhtOpHash)>> {
    vault
        .read_async(move |txn| get_pending_validation_receipts(&txn, validators))
        .await
}

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

    // If we fail to connect to an author once during the workflow run, don't try again. The send already has a
    // retry mechanism so it doesn't make sense to keep sending receipts to an author we can't connect to.
    let mut unavailable_authors = HashSet::new();

    // Try to send the validation receipts
    for (receipt, author, dht_op_hash) in receipts {
        if !unavailable_authors.contains(&author) {
            match sign_and_send_receipt(
                &dna_hash,
                &network,
                &keystore,
                &validators,
                &author,
                receipt,
                apply_block.clone(),
            )
            .await
            {
                Ok(SendOutcome::Attempted) => {
                    // Success, nothing more to do
                }
                Ok(SendOutcome::AuthorUnavailable) => {
                    unavailable_authors.insert(author);
                }
                Err(e) => {
                    info!(failed_to_sign_and_send_receipt = ?e);
                }
            }
        } else {
            info!(
                "Skipping sending validation receipt to {:?} because they are unavailable: {:?}",
                author, receipt
            );
        }

        // Attempted to send the receipt so we now mark it to not send in the future.
        vault
            .write_async(move |txn| set_require_receipt(txn, &dht_op_hash, false))
            .await?;
    }

    Ok(WorkComplete::Complete)
}

async fn sign_and_send_receipt<B>(
    dna_hash: &DnaHash,
    network: &impl HolochainP2pDnaT,
    keystore: &MetaLairClient,
    validators: &HashSet<AgentPubKey>,
    op_author: &AgentPubKey,
    receipt: ValidationReceipt,
    apply_block: B,
) -> WorkflowResult<SendOutcome>
where
    B: Fn(Block) -> BoxFuture<'static, DatabaseResult<()>>,
{
    // Don't send receipt to self. Don't block self.
    if validators.contains(op_author) {
        return Ok(SendOutcome::Attempted);
    }

    // Block authors of invalid ops.
    if matches!(receipt.validation_status, ValidationStatus::Rejected) {
        // Block BEFORE we integrate the outcome because this is not atomic
        // and if something goes wrong we know the integration will retry.
        apply_block(Block::new(
            BlockTarget::Cell(
                CellId::new((*dna_hash).clone(), op_author.clone()),
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
            // This branch should not be reachable, log an error if we somehow hit it to help diagnose the problem.
            error!("No agents found to sign the validation receipt after checking that there was at least 1: {:?}", receipt);
            return Ok(SendOutcome::Attempted);
        }
        Err(e) => {
            // TODO Which errors are retryable here? A fatal error would keep being retried and we don't want that;
            //      aggressively give up for now.
            info!(failed_to_sign_receipt = ?e);
            return Ok(SendOutcome::Attempted);
        }
    };

    // Send it and don't wait for response.
    match holochain_p2p::HolochainP2pDnaT::send_validation_receipt(
        network,
        op_author.clone(),
        receipt.try_into()?,
    )
    .await
    {
        Ok(_) => Ok(SendOutcome::Attempted),
        Err(e) => {
            // No one home, they will need to publish again.
            info!(failed_send_receipt = ?e);
            Ok(SendOutcome::AuthorUnavailable)
        }
    }
}

async fn pending_receipts(
    vault: &DbRead<DbKindDht>,
    validators: Vec<AgentPubKey>,
) -> StateQueryResult<Vec<(ValidationReceipt, AgentPubKey, DhtOpHash)>> {
    vault
        .read_async(move |txn| get_pending_validation_receipts(&txn, validators))
        .await
}
