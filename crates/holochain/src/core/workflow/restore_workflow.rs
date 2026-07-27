//! This workflow reconstructs an agent's source chain from the DHT in place of genesis for a cell
//! installed with `restore_from_dht: true`. The workflow itself loops internally, retrying with a
//! backoff, until it reaches a terminal [`RestoreOutcome`].
//!
//! Each attempt of the workflow follows these distinct steps:
//!
//!
//! * **Step 1**, in [`agent_activity`], gets the agent's chain activity from the DHT, aggregates
//!   the responses, requires unanimous agreement on the chain head from the peers that responded,
//!   then collects the verified `Record`s. It also collects any warrants naming the agent
//!   regardless of whether a head was agreed this round.
//! * **Step 2**, in [`warrants`], runs only when responses from Step 1 include warrants against the
//!   agent whose chain is being restored. It stages the received warrants for local validation and
//!   polls for a verdict. If any single warrant is validated then the restore will fail permanently
//!   for this cell. If no warrants were received or all warrants are rejected then the attempt
//!   proceeds to Step 3 using the chain head that was agreed upon in Step 1.
//! * **Step 3**, in [`chain_reconstruction`], walks the collected records backward from the agreed
//!   head to genesis, then writes the verified chain directly into the per-DNA database as authored
//!   state, this bypasses validation limbo.
//!
//! Reporting completion to the per-app orchestrator, emitting system signals, and app status
//! transitions are the responsibility of the orchestrator.

use std::time::Duration;

use holochain_cascade::CascadeImpl;
use holochain_state::dht_store::DhtStore;
use holochain_types::prelude::*;
use tokio::time::sleep;

use crate::core::workflow::error::WorkflowResult;

use agent_activity::AcquireOutcome;
use chain_reconstruction::ReconstructionOutcome;
use warrants::WarrantOutcome;

pub(crate) mod agent_activity;
pub(crate) mod chain_reconstruction;
pub(crate) mod warrants;

/// The outcome of a full restore attempt for a single cell.
pub(crate) enum RestoreOutcome {
    /// The chain was reconstructed and written to the per-DNA database.
    Complete,
    /// A validated warrant proves the agent's chain is compromised and cannot be restored.
    PermanentFailure(UnrecoverableCellReason),
}

/// Reconstructs `cell_id`'s source chain from the DHT, retrying with `retry_delay` backoff until
/// the chain is written or a validated warrant proves it unrecoverable.
pub(crate) async fn restore_workflow(
    cell_id: CellId,
    cascade: CascadeImpl,
    dht_store: DhtStore,
    quorum: u8,
    retry_delay: Duration,
) -> WorkflowResult<RestoreOutcome> {
    loop {
        let (outcome, warrants) =
            agent_activity::acquire_responses(&cascade, cell_id.agent_pubkey(), quorum).await?;

        if !warrants.is_empty() {
            loop {
                match warrants::stage_and_check_warrants(&dht_store, warrants.clone()).await? {
                    WarrantOutcome::Warranted(reason) => {
                        return Ok(RestoreOutcome::PermanentFailure(reason));
                    }
                    WarrantOutcome::Pending => sleep(retry_delay).await,
                    WarrantOutcome::Cleared => break,
                }
            }
        }

        match outcome {
            AcquireOutcome::Agreed {
                head_seq,
                head_hash,
                records,
            } => {
                tracing::debug!(head_seq, ?head_hash, ?cell_id, "Restore: chain head agreed");
                if let ReconstructionOutcome::Complete(chain) =
                    chain_reconstruction::reconstruct_chain(records, &head_hash)
                {
                    dht_store
                        .write_restored_chain(cell_id.agent_pubkey(), chain)
                        .await?;
                    return Ok(RestoreOutcome::Complete);
                };

                tracing::debug!(?cell_id, "Restore: chain reconstruction failed, retrying");
            }
            AcquireOutcome::Retry(reason) => {
                tracing::debug!(?reason, ?cell_id, "Restore: retrying");
            }
        }

        sleep(retry_delay).await;
    }
}
