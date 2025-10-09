use super::error::WorkflowResult;
use crate::core::queue_consumer::WorkComplete;
use futures::{stream, StreamExt};
use holochain_keystore::MetaLairClient;
use holochain_p2p::DynHolochainP2pDna;
use holochain_state::prelude::*;
use itertools::Itertools;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod unit_tests;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(vault, network, keystore, apply_block))
)]
/// Send validation receipts to their authors in serial and without waiting for responses.
pub async fn validation_receipt_workflow(
    dna_hash: Arc<DnaHash>,
    vault: DbWrite<DbKindDht>,
    network: DynHolochainP2pDna,
    keystore: MetaLairClient,
    running_cell_ids: HashSet<CellId>,
) -> WorkflowResult<WorkComplete> {
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
        .chunk_by(|(_, author)| author.clone())
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
            network.clone(),
            &keystore,
            &validators,
            &author,
            receipts.clone(),
        )
        .await
        {
            Ok(()) => {
                // Mark them sent so we don't keep trying
                for receipt in receipts {
                    vault
                        .write_async(move |txn| {
                            set_require_receipt(txn, &receipt.dht_op_hash, false)
                        })
                        .await?;
                }
            }
            Err(e) => {
                info!(failed_to_sign_and_send_receipt = ?e);
            }
        }
    }

    Ok(WorkComplete::Complete)
}

/// Perform the signing and sending of
/// Requires that the receipts to send are all by the same author.
async fn sign_and_send_receipts_to_author(
    network: DynHolochainP2pDna,
    keystore: &MetaLairClient,
    validators: &HashSet<AgentPubKey>,
    op_author: &AgentPubKey,
    receipts: Vec<ValidationReceipt>,
) -> WorkflowResult<()> {
    // Don't send receipt to self. Don't block self.
    if validators.contains(op_author) {
        return Ok(());
    }

    let num_receipts = receipts.len();

    let receipts: Vec<SignedValidationReceipt> = stream::iter(receipts)
        .filter_map(|receipt| async {
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

    // Actually send the receipt to the author.
    holochain_p2p::HolochainP2pDnaT::send_validation_receipts(
        network.as_ref(),
        op_author.clone(),
        receipts.into(),
    )
    .await?;

    Ok(())
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
async fn pending_receipts(
    vault: &DbRead<DbKindDht>,
    validators: Vec<AgentPubKey>,
) -> StateQueryResult<Vec<(ValidationReceipt, AgentPubKey)>> {
    vault
        .read_async(move |txn| get_pending_validation_receipts(txn, validators))
        .await
}
