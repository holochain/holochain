//! Part of the restore workflow. Stages warrants raised against the restoring agent for local
//! validation, then reports whether local validation has found the agent's chain compromised.

use holo_hash::DhtOpHash;
use holochain_state::dht_store::DhtStore;
use holochain_types::prelude::{UnrecoverableCellReason, WarrantSummary};
use holochain_types::warrant::WarrantOp;
use holochain_zome_types::prelude::{
    ChainIntegrityWarrant, SignedWarrant, ValidationStatus, WarrantProof,
};

use crate::core::workflow::error::WorkflowResult;

/// The result of staging a set of warrants raised against the restoring agent and checking
/// local validation's verdict on each of them.
#[derive(Debug)]
pub(super) enum WarrantOutcome {
    /// Every warrant was rejected by local validation. The agent's chain is not proven
    /// compromised and restore can proceed.
    Cleared,
    /// At least one warrant is still awaiting a terminal verdict from local validation.
    Pending,
    /// A locally-validated [`ChainIntegrityWarrant`] proves the agent's chain is compromised.
    /// The restore for this cell cannot proceed.
    Warranted(UnrecoverableCellReason),
}

/// Stages warrants for local validation, then checks each one's validation status.
///
/// Staging is idempotent: a warrant already in limbo or already integrated is left untouched by
/// the repeated submission.
///
/// # Returns
/// * [`WarrantOutcome::Pending`] whilst any warrant lacks a terminal verdict
/// * [`WarrantOutcome::Warranted`] on the first valid warrant found
/// * [`WarrantOutcome::Cleared`] only once every warrant has an invalid terminal verdict
pub(super) async fn stage_and_check_warrants(
    dht_store: &DhtStore,
    warrants: Vec<SignedWarrant>,
) -> WorkflowResult<WarrantOutcome> {
    let warrant_ops: Vec<WarrantOp> = warrants.into_iter().map(WarrantOp::from).collect();
    dht_store
        .stage_warrants_for_validation(warrant_ops.clone())
        .await?;

    let reader = dht_store.as_read();
    let mut any_pending = false;
    for op in &warrant_ops {
        let op_hash = DhtOpHash::with_data_sync(op);
        match reader.warrant_validation_status(&op_hash).await? {
            None => any_pending = true,
            Some(ValidationStatus::Valid) => {
                return Ok(WarrantOutcome::Warranted(unrecoverable_reason(op)));
            }
            Some(_) => {}
        }
    }

    if any_pending {
        Ok(WarrantOutcome::Pending)
    } else {
        Ok(WarrantOutcome::Cleared)
    }
}

/// Maps a validated warrant to the matching [`UnrecoverableCellReason`].
///
/// [`ChainIntegrityWarrant::ChainFork`] is reported distinctly from every other
/// [`ChainIntegrityWarrant`] variant.
fn unrecoverable_reason(warrant: &WarrantOp) -> UnrecoverableCellReason {
    let summary = WarrantSummary {
        author: warrant.author.clone(),
        warrantee: warrant.warrantee.clone(),
        timestamp: warrant.timestamp,
    };
    match warrant.proof {
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork { .. }) => {
            UnrecoverableCellReason::ChainForkWarrant(Box::new(summary))
        }
        WarrantProof::ChainIntegrity(_) => {
            UnrecoverableCellReason::ChainIntegrityWarrant(Box::new(summary))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey};
    use holochain_types::dht_op::DhtOp;
    use holochain_types::dht_op::DhtOpHashed;
    use holochain_zome_types::op::ChainOpType;
    use holochain_zome_types::prelude::{Signature, Timestamp, Warrant};

    fn dht_id() -> holochain_state::data::Dht {
        holochain_state::data::Dht::new(std::sync::Arc::new(holo_hash::DnaHash::from_raw_36(
            vec![0u8; 36],
        )))
    }

    fn build_chain_fork_warrant(seed: u8) -> SignedWarrant {
        let chain_author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let action_a = ActionHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let action_b = ActionHash::from_raw_36(vec![seed.wrapping_add(101); 36]);
        SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
                    chain_author,
                    action_pair: (
                        (action_a, Signature::from([seed; 64])),
                        (action_b, Signature::from([seed.wrapping_add(1); 64])),
                    ),
                    seq: 0,
                }),
                AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
                Timestamp::from_micros(seed as i64 * 1000),
                AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]),
            ),
            Signature::from([seed.wrapping_add(2); 64]),
        )
    }

    fn build_invalid_chain_op_warrant(seed: u8) -> SignedWarrant {
        let action_author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let action_hash = ActionHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author,
                    action: (action_hash, Signature::from([seed; 64])),
                    chain_op_type: ChainOpType::StoreRecord,
                    reason: "test warrant".into(),
                }),
                AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
                Timestamp::from_micros(seed as i64 * 1000),
                AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]),
            ),
            Signature::from([seed.wrapping_add(2); 64]),
        )
    }

    #[test]
    fn unrecoverable_reason_maps_chain_fork_distinctly() {
        let op = WarrantOp::from(build_chain_fork_warrant(1));
        let reason = unrecoverable_reason(&op);
        assert!(matches!(
            reason,
            UnrecoverableCellReason::ChainForkWarrant(_)
        ));
    }

    #[test]
    fn unrecoverable_reason_maps_other_chain_integrity_variants() {
        let op = WarrantOp::from(build_invalid_chain_op_warrant(2));
        let reason = unrecoverable_reason(&op);
        assert!(matches!(
            reason,
            UnrecoverableCellReason::ChainIntegrityWarrant(_)
        ));
    }

    #[tokio::test]
    async fn no_warrants_returns_cleared() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();

        let outcome = stage_and_check_warrants(&store, vec![]).await.unwrap();
        assert!(matches!(outcome, WarrantOutcome::Cleared));
    }

    #[tokio::test]
    async fn unintegrated_warrant_is_pending() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant = build_chain_fork_warrant(5);

        let outcome = stage_and_check_warrants(&store, vec![warrant])
            .await
            .unwrap();
        assert!(matches!(outcome, WarrantOutcome::Pending));
    }

    #[tokio::test]
    async fn integrated_valid_warrant_is_failed() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant = build_chain_fork_warrant(6);
        let warrant_op = WarrantOp::from(warrant.clone());
        let hashed = DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(warrant_op)));

        store.test_insert_integrated_warrant(hashed).await.unwrap();

        let outcome = stage_and_check_warrants(&store, vec![warrant])
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            WarrantOutcome::Warranted(UnrecoverableCellReason::ChainForkWarrant(_))
        ));
    }

    #[tokio::test]
    async fn valid_warrant_wins_even_when_listed_after_a_pending_one() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();

        // Stays in limbo: never integrated.
        let pending = build_invalid_chain_op_warrant(7);
        // Seeded straight into the integrated, Valid state.
        let valid = build_chain_fork_warrant(8);
        let valid_op = WarrantOp::from(valid.clone());
        let hashed = DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(valid_op)));
        store.test_insert_integrated_warrant(hashed).await.unwrap();

        // The pending warrant is listed first, so a naive first-found scan would return
        // `Pending` without ever looking at the already-validated warrant that follows it.
        let outcome = stage_and_check_warrants(&store, vec![pending, valid])
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            WarrantOutcome::Warranted(UnrecoverableCellReason::ChainForkWarrant(_))
        ));
    }
}
