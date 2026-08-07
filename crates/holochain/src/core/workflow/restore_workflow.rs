//! This workflow reconstructs an agent's source chain from the DHT in place of genesis for a cell
//! installed with `restore_from_dht: true`. The workflow itself loops internally, retrying with a
//! backoff, until it reaches a terminal `RestoreOutcome`.
//!
//! Each attempt follows two steps:
//!
//! * **Step 1**, in `agent_activity`, gets the agent's chain activity from the DHT, aggregates the
//!   responses, requires unanimous agreement on the chain head from the peers that responded, then
//!   collects the verified `Record`s. It also collects any warrants naming the agent regardless of
//!   whether or not a head was agreed this round. When warrants are present they are staged for
//!   local validation and polled for a verdict before the attempt can proceed. If any single
//!   warrant is validated then the restore fails permanently for this cell. If no warrants are
//!   found to be valid then the attempt proceeds using the chain head agreed upon earlier in this
//!   step.
//! * **Step 2**, in `chain_reconstruction`, walks the collected records backward from the agreed
//!   head to genesis, then writes the verified chain directly into the per-DNA database as authored
//!   state, this bypasses validation limbo.

use std::time::Duration;

use holochain_cascade::CascadeImpl;
use holochain_state::dht_store::DhtStore;
use holochain_types::prelude::*;
use tokio::time::sleep;

use crate::core::queue_consumer::TriggerSender;
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
///
/// Without a valid `sys_validation_trigger`, any staged warrants won't reach a verdict and
/// the restore workflow would just loop forever without resolving.
pub(crate) async fn restore_workflow(
    cell_id: CellId,
    cascade: CascadeImpl,
    dht_store: DhtStore,
    quorum: u8,
    retry_delay: Duration,
    sys_validation_trigger: TriggerSender,
) -> WorkflowResult<RestoreOutcome> {
    loop {
        let (outcome, warrants) =
            agent_activity::acquire_responses(&cascade, cell_id.agent_pubkey(), quorum).await?;

        if !warrants.is_empty() {
            warrants::stage_warrants(&dht_store, &warrants).await?;
            loop {
                sys_validation_trigger.trigger(&"restore_workflow");
                sleep(retry_delay).await;
                match warrants::check_warrant_status(&dht_store, &warrants).await? {
                    WarrantOutcome::Warranted(reason) => {
                        return Ok(RestoreOutcome::PermanentFailure(reason));
                    }
                    WarrantOutcome::Pending => {}
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
                    chain_reconstruction::reconstruct_chain(records, head_seq, &head_hash)
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator, DnaHashFixturator};
    use holochain_keystore::{test_keystore, AgentPubKeyExt};
    use holochain_p2p::{DynHolochainP2pDna, MockHolochainP2pDnaT};
    use holochain_types::activity::{AgentActivityResponse, ChainItems};
    use holochain_types::op::{DhtOp, DhtOpHashed};
    use holochain_types::warrant::WarrantOp;
    use holochain_zome_types::prelude::*;
    use holochain_zome_types::query::{ChainHead, ChainStatus};

    use super::*;

    fn dht_id() -> holochain_state::data::Dht {
        holochain_state::data::Dht::new(Arc::new(holo_hash::DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn valid_head(seq: u32, hash: ActionHash) -> ChainStatus {
        ChainStatus::Valid(ChainHead {
            action_seq: seq,
            hash,
        })
    }

    fn make_response(
        agent: &AgentPubKey,
        status: ChainStatus,
        records: Vec<Record>,
        warrants: Vec<SignedWarrant>,
    ) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::Full(records),
            rejected_activity: ChainItems::Full(vec![]),
            status,
            highest_observed: None,
            warrants,
        }
    }

    /// Builds a genuinely-signed chain of `len` records for `agent`, so that
    /// `acquire_responses`'s signature filter keeps every record.
    async fn build_chain(
        keystore: &holochain_keystore::MetaLairClient,
        agent: &AgentPubKey,
        len: u32,
    ) -> Vec<Record> {
        let mut records = Vec::new();
        let mut prev_hash: Option<ActionHash> = None;
        for seq in 0..len {
            let data = if prev_hash.is_none() {
                ActionData::Dna(DnaData {
                    dna_hash: ::fixt::fixt!(DnaHash),
                })
            } else {
                ActionData::InitZomesComplete(InitZomesCompleteData {})
            };
            let action = Action {
                header: ActionHeader {
                    author: agent.clone(),
                    timestamp: Timestamp::from_micros(seq as i64 * 1000),
                    action_seq: seq,
                    prev_action: prev_hash.clone(),
                },
                data,
            };
            let signature = agent.sign(keystore, action.clone()).await.unwrap();
            let action_hashed = ActionHashed::from_content_sync(action);
            prev_hash = Some(action_hashed.as_hash().clone());
            let signed = SignedActionHashed::with_presigned(action_hashed, signature);
            records.push(Record::new(signed, RecordEntry::NA));
        }
        records
    }

    fn make_signed_warrant(agent: &AgentPubKey) -> SignedWarrant {
        let proof = WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
            chain_author: agent.clone(),
            action_pair: (
                (::fixt::fixt!(ActionHash), ::fixt::fixt!(Signature)),
                (::fixt::fixt!(ActionHash), ::fixt::fixt!(Signature)),
            ),
            seq: 0,
        });
        let warrant = Warrant::new(
            proof,
            ::fixt::fixt!(AgentPubKey),
            Timestamp::from_micros(0),
            agent.clone(),
        );
        SignedWarrant::new(warrant, ::fixt::fixt!(Signature))
    }

    /// A mock network that returns one canned response, from a single fixed peer, per call,
    /// advancing through `responses` in order and repeating the last one once exhausted.
    fn mock_network(responses: Vec<AgentActivityResponse>) -> DynHolochainP2pDna {
        let call = Arc::new(AtomicUsize::new(0));
        let peer = ::fixt::fixt!(AgentPubKey);
        let mut mock = MockHolochainP2pDnaT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(true));
        mock.expect_get_agent_activity_multi()
            .returning(move |_, _, _| {
                let i = call
                    .fetch_add(1, Ordering::Relaxed)
                    .min(responses.len() - 1);
                Ok(vec![(peer.clone(), responses[i].clone())])
            });
        Arc::new(mock)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn retries_on_no_activity_then_succeeds() {
        let keystore = test_keystore();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();
        let cell_id = CellId::new(::fixt::fixt!(DnaHash), agent.clone());

        let chain = build_chain(&keystore, &agent, 3).await;
        let head_hash = chain.last().unwrap().action_address().clone();

        let network = mock_network(vec![
            make_response(&agent, ChainStatus::Empty, vec![], vec![]),
            make_response(&agent, valid_head(2, head_hash), chain, vec![]),
        ]);
        let dht_store = DhtStore::new_test(dht_id()).await.unwrap();
        let cascade = CascadeImpl::empty(dht_store.clone()).with_network(network);

        let outcome = restore_workflow(
            cell_id,
            cascade,
            dht_store,
            1,
            Duration::from_millis(1),
            TriggerSender::new().0,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, RestoreOutcome::Complete));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn permanent_failure_on_validated_warrant() {
        let keystore = test_keystore();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();
        let cell_id = CellId::new(::fixt::fixt!(DnaHash), agent.clone());

        let dht_store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant = make_signed_warrant(&agent);
        let hashed = DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(WarrantOp::from(
            warrant.clone(),
        ))));
        dht_store
            .test_insert_integrated_warrant_with_status(hashed, RecordValidity::Accepted)
            .await
            .unwrap();

        let chain = build_chain(&keystore, &agent, 3).await;
        let head_hash = chain.last().unwrap().action_address().clone();
        let network = mock_network(vec![make_response(
            &agent,
            valid_head(2, head_hash),
            chain,
            vec![warrant],
        )]);
        let cascade = CascadeImpl::empty(dht_store.clone()).with_network(network);

        let outcome = restore_workflow(
            cell_id,
            cascade,
            dht_store,
            1,
            Duration::from_millis(1),
            TriggerSender::new().0,
        )
        .await
        .unwrap();
        assert!(matches!(
            outcome,
            RestoreOutcome::PermanentFailure(UnrecoverableCellReason::ChainForkWarrant(_))
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cleared_warrant_then_succeeds() {
        let keystore = test_keystore();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();
        let cell_id = CellId::new(::fixt::fixt!(DnaHash), agent.clone());

        let dht_store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant = make_signed_warrant(&agent);
        let hashed = DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(WarrantOp::from(
            warrant.clone(),
        ))));
        dht_store
            .test_insert_integrated_warrant_with_status(hashed, RecordValidity::Rejected)
            .await
            .unwrap();

        let chain = build_chain(&keystore, &agent, 3).await;
        let head_hash = chain.last().unwrap().action_address().clone();
        let network = mock_network(vec![make_response(
            &agent,
            valid_head(2, head_hash),
            chain,
            vec![warrant],
        )]);
        let cascade = CascadeImpl::empty(dht_store.clone()).with_network(network);

        let outcome = restore_workflow(
            cell_id,
            cascade,
            dht_store,
            1,
            Duration::from_millis(1),
            TriggerSender::new().0,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, RestoreOutcome::Complete));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn incomplete_reconstruction_retries_then_completes() {
        let keystore = test_keystore();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();
        let cell_id = CellId::new(::fixt::fixt!(DnaHash), agent.clone());

        let chain = build_chain(&keystore, &agent, 3).await;
        let head_hash = chain.last().unwrap().action_address().clone();
        let mut gappy_chain = chain.clone();
        gappy_chain.remove(1);

        let network = mock_network(vec![
            make_response(
                &agent,
                valid_head(2, head_hash.clone()),
                gappy_chain,
                vec![],
            ),
            make_response(&agent, valid_head(2, head_hash), chain, vec![]),
        ]);
        let dht_store = DhtStore::new_test(dht_id()).await.unwrap();
        let cascade = CascadeImpl::empty(dht_store.clone()).with_network(network);

        let outcome = restore_workflow(
            cell_id,
            cascade,
            dht_store,
            1,
            Duration::from_millis(1),
            TriggerSender::new().0,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, RestoreOutcome::Complete));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn forged_record_from_one_peer_dropped_restore_succeeds_via_other_peer_same_round() {
        let keystore = test_keystore();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();
        let cell_id = CellId::new(::fixt::fixt!(DnaHash), agent.clone());

        let chain = build_chain(&keystore, &agent, 3).await;
        let head_hash = chain.last().unwrap().action_address().clone();

        // Peer B has a record with a signature that the agent never produced, acting as a
        // misbehaving peer relaying a corrupted record.
        let mut tampered_chain = chain.clone();
        let tampered_action = ActionHashed::from_content_sync(tampered_chain[1].action().clone());
        tampered_chain[1] = Record::new(
            SignedActionHashed::with_presigned(tampered_action, ::fixt::fixt!(Signature)),
            RecordEntry::NA,
        );

        let peer_a = ::fixt::fixt!(AgentPubKey);
        let peer_b = ::fixt::fixt!(AgentPubKey);
        let response_a = make_response(&agent, valid_head(2, head_hash.clone()), chain, vec![]);
        let response_b = make_response(
            &agent,
            valid_head(2, head_hash.clone()),
            tampered_chain,
            vec![],
        );

        let mut mock = MockHolochainP2pDnaT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(true));
        mock.expect_get_agent_activity_multi()
            .returning(move |_, _, _| {
                Ok(vec![
                    (peer_a.clone(), response_a.clone()),
                    (peer_b.clone(), response_b.clone()),
                ])
            });
        let network: DynHolochainP2pDna = Arc::new(mock);

        let dht_store = DhtStore::new_test(dht_id()).await.unwrap();
        let cascade = CascadeImpl::empty(dht_store.clone()).with_network(network);

        // A quorum of 2 requires that both peers to respond in the same round, and as the mock only
        // ever returns these responses, then a success can only come from combining them and not
        // from a later retry against different data.
        let outcome = restore_workflow(
            cell_id,
            cascade,
            dht_store,
            2,
            Duration::from_millis(1),
            TriggerSender::new().0,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, RestoreOutcome::Complete));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn happy_path_writes_and_completes() {
        let keystore = test_keystore();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();
        let cell_id = CellId::new(::fixt::fixt!(DnaHash), agent.clone());

        let chain = build_chain(&keystore, &agent, 3).await;
        let head_hash = chain.last().unwrap().action_address().clone();
        let network = mock_network(vec![make_response(
            &agent,
            valid_head(2, head_hash),
            chain,
            vec![],
        )]);
        let dht_store = DhtStore::new_test(dht_id()).await.unwrap();
        let cascade = CascadeImpl::empty(dht_store.clone()).with_network(network);

        let outcome = restore_workflow(
            cell_id,
            cascade,
            dht_store,
            1,
            Duration::from_millis(1),
            TriggerSender::new().0,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, RestoreOutcome::Complete));
    }
}
