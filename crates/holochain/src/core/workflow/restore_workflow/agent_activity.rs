//! Part of the restore workflow. Acquires the agent's chain from the DHT and pins the target chain
//! head by requiring unanimous agreement across responses. Warrants naming the agent are also
//! collected alongside the head agreement, since a validated warrant is grounds for permanent
//! failure regardless of whether or not a chain head could be agreed.

use holo_hash::{ActionHash, AgentPubKey};
use holochain_cascade::{error::CascadeError, CascadeImpl};
use holochain_keystore::AgentPubKeyExt;
use holochain_p2p::{actor::GetActivityMultiOptions, event::GetActivityOptions, HolochainP2pError};
use holochain_types::activity::{AgentActivityResponse, ChainItems};
use holochain_zome_types::{
    prelude::{Record, SignedWarrant},
    query::{ChainQueryFilter, ChainStatus},
};

use crate::core::workflow::{error::WorkflowResult, WorkflowError};

/// The result of one attempt to acquire the agent's chain activity from the DHT.
#[derive(Debug)]
pub(super) enum AcquireOutcome {
    /// Enough peer responses arrived and all agreed on the same chain head.
    Agreed {
        /// Sequence number of the agreed chain head.
        head_seq: u32,
        /// Hash of the agreed chain head action.
        head_hash: ActionHash,
        /// [`Record`]s collected from every response whose signature verified against `agent`.
        records: Vec<Record>,
    },
    /// The acquisition could not agree on a chain head, check the inner [`RetryReason`] for
    /// details. A fresh attempt should be made after a backoff delay.
    Retry(RetryReason),
}

/// The reason why a chain head could not be agreed upon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RetryReason {
    /// Not enough peers responded to meet the configured minimum.
    TooFewResponses {
        /// Number of responses received.
        got: usize,
        /// Minimum number required.
        need: u8,
    },
    /// The peer responses disagreed on the chain head, or at least one peer reported a
    /// [`ChainStatus::Forked`] status.
    HeadDisagreement,
    /// Every peer response reports [`ChainStatus::Empty`] meaning that either the agent's chain
    /// does not yet exist on the DHT or there is no peer connectivity.
    NoActivity,
}

/// Queries peers to get the agent's full chain activity from the DHT, evaluates the responses for
/// quorum and chain-head agreement to produce an [`AcquireOutcome`], and collects any warrants
/// against `agent`, regardless of the head outcome.
/// When agreement is reached it filters the collected records for those with a valid signature
/// against `agent`.
pub(super) async fn acquire_responses(
    cascade: &CascadeImpl,
    agent: &AgentPubKey,
    quorum: u8,
) -> WorkflowResult<(AcquireOutcome, Vec<SignedWarrant>)> {
    let options = GetActivityMultiOptions {
        target_peer_count: quorum.saturating_add(1),
        required_responses: quorum,
        timeout_ms: None,
        remote_options: GetActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: true,
            include_warrants: true,
            include_full_records: true,
        },
    };
    let query = ChainQueryFilter::new().include_entries(true);
    let responses = match cascade
        .get_agent_activity_multi(agent.clone(), query, options)
        .await
    {
        Ok(responses) => responses
            .into_iter()
            .map(|(_, response)| response)
            .collect(),
        // Not enough peers held data for this agent within the timeout. This could mean that the
        // agent's chain hasn't been gossiped yet, or peers are unreachable. Therefore, we should
        // retry instead of a hard error.
        Err(CascadeError::NetworkError(HolochainP2pError::InsufficientResponses {
            received,
            required,
            ..
        })) => {
            return Ok((
                AcquireOutcome::Retry(RetryReason::TooFewResponses {
                    got: received,
                    need: required as u8,
                }),
                Vec::new(),
            ));
        }
        Err(err) => return Err(WorkflowError::CascadeError(err)),
    };

    let (mut outcome, warrants) = evaluate_responses(agent, responses, quorum);

    // If the peers agreed on a head, filter the collected records by signature.
    // Records whose signature fails verification are dropped so that a misbehaving peer cannot
    // abort the restore by serving forgeries.
    if let AcquireOutcome::Agreed { records, .. } = &mut outcome {
        *records = filter_records_by_author_and_signature(agent, std::mem::take(records)).await;
    }

    Ok((outcome, warrants))
}

/// Evaluates a set of [`AgentActivityResponse`]s and produces an [`AcquireOutcome`] by checking
/// quorum, requiring unanimous chain-head agreement, and collecting the full records. Warrants
/// naming `agent` are also collected and returned alongside the outcome regardless of whether a
/// chain head was agreed upon or not as a validated warrant is grounds for permanent failure no
/// matter the outcome of the chain-head agreement. No signature filtering is applied on the
/// returned records so this should be done by the caller.
pub(super) fn evaluate_responses(
    agent: &AgentPubKey,
    responses: Vec<AgentActivityResponse>,
    quorum: u8,
) -> (AcquireOutcome, Vec<SignedWarrant>) {
    if responses.len() < quorum as usize {
        return (
            AcquireOutcome::Retry(RetryReason::TooFewResponses {
                got: responses.len(),
                need: quorum,
            }),
            Vec::new(),
        );
    }

    // Collect warrants naming this agent from every response.
    let warrants_for_agent: Vec<SignedWarrant> = responses
        .iter()
        .flat_map(|resp| resp.warrants.iter())
        .filter(|warrant| warrant.warrantee == *agent)
        .cloned()
        .collect();

    // Determine the agreed chain head. Every non-empty response must share the same
    // (action_seq, hash) pair.
    let mut agreed_head = None;
    for response in &responses {
        match &response.status {
            ChainStatus::Empty => {}

            ChainStatus::Valid(head) | ChainStatus::Invalid(head) | ChainStatus::Closed(head) => {
                if let Some((prev_seq, prev_hash)) = &agreed_head {
                    if &head.action_seq != prev_seq || &head.hash != prev_hash {
                        return (
                            AcquireOutcome::Retry(RetryReason::HeadDisagreement),
                            warrants_for_agent,
                        );
                    }
                } else {
                    agreed_head = Some((head.action_seq, head.hash.clone()));
                }
            }

            // A fork status means that the peer cannot single out a single chain head, so this is
            // treated as a disagreement, prompting the workflow to retry.
            ChainStatus::Forked(_) => {
                return (
                    AcquireOutcome::Retry(RetryReason::HeadDisagreement),
                    warrants_for_agent,
                );
            }
        }
    }

    let Some((head_seq, head_hash)) = agreed_head else {
        return (
            AcquireOutcome::Retry(RetryReason::NoActivity),
            warrants_for_agent,
        );
    };

    // Gather all full records from every response, we only enforce that the record's author field
    // matches `agent`. Filtering on signatures should be applied by the caller.
    let records: Vec<Record> = responses
        .into_iter()
        .flat_map(|r| {
            if let ChainItems::Full(recs) = r.valid_activity {
                recs
            } else {
                Vec::new()
            }
        })
        .filter(|r| r.action().author() == agent)
        .collect();

    (
        AcquireOutcome::Agreed {
            head_seq,
            head_hash,
            records,
        },
        warrants_for_agent,
    )
}

/// Retain only the records whose action signature verifies against `agent`.
///
/// Records authored by a different agent or carrying a bad signature are silently dropped.
/// This means that misbehaving peers that serve forged actions cannot abort the restore.
async fn filter_records_by_author_and_signature(
    agent: &AgentPubKey,
    records: Vec<Record>,
) -> Vec<Record> {
    let mut verified = Vec::with_capacity(records.len());
    for record in records {
        // Author must match the restoring agent.
        if record.action().author() != agent {
            tracing::warn!(
                author = ?record.action().author(),
                expected = ?agent,
                "Restore: record from wrong author, discarding"
            );
            continue;
        }

        // Verify the action's signature against the agent's key.
        let action = record.action();
        match agent.verify_signature(record.signature(), action).await {
            Ok(true) => verified.push(record),
            Ok(false) => {
                tracing::warn!(
                    seq = action.action_seq(),
                    "Restore: record signature check failed, discarding"
                );
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    seq = action.action_seq(),
                    "Restore: error verifying record signature, discarding"
                );
            }
        }
    }
    verified
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator, DnaHashFixturator};
    use holochain_keystore::AgentPubKeyExt;
    use holochain_types::activity::ChainItems;
    use holochain_zome_types::prelude::*;
    use holochain_zome_types::query::{ChainHead, ChainStatus};

    fn make_response(agent: &AgentPubKey, status: ChainStatus) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status,
            highest_observed: None,
            warrants: vec![],
        }
    }

    fn make_response_with_warrants(
        agent: &AgentPubKey,
        status: ChainStatus,
        warrants: Vec<SignedWarrant>,
    ) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status,
            highest_observed: None,
            warrants,
        }
    }

    fn valid_head(seq: u32, hash: ActionHash) -> ChainStatus {
        ChainStatus::Valid(ChainHead {
            action_seq: seq,
            hash,
        })
    }

    fn make_record_for_agent(agent: &AgentPubKey) -> Record {
        use ::fixt::prelude::*;
        let action = Action {
            header: ActionHeader {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(0),
                action_seq: 0,
                prev_action: None,
            },
            data: ActionData::Dna(DnaData {
                dna_hash: fixt!(DnaHash),
            }),
        };
        let action_hashed = ActionHashed::from_content_sync(action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        Record::new(signed, RecordEntry::NA)
    }

    fn make_response_with_records(
        agent: &AgentPubKey,
        status: ChainStatus,
        records: Vec<Record>,
    ) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::Full(records),
            rejected_activity: ChainItems::NotRequested,
            status,
            highest_observed: None,
            warrants: vec![],
        }
    }

    fn make_signed_warrant(agent: &AgentPubKey) -> SignedWarrant {
        use ::fixt::prelude::*;
        let proof = WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
            chain_author: agent.clone(),
            action_pair: (
                (fixt!(ActionHash), fixt!(Signature)),
                (fixt!(ActionHash), fixt!(Signature)),
            ),
            seq: 0,
        });
        let warrant = Warrant::new(
            proof,
            fixt!(AgentPubKey), // author (the warranter)
            Timestamp::from_micros(0),
            agent.clone(), // warrantee
        );
        SignedWarrant::new(warrant, fixt!(Signature))
    }

    #[test]
    fn insufficient_responses_returns_retry() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let responses = vec![make_response(&agent, valid_head(5, hash))];
        let (outcome, warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::TooFewResponses { got: 1, need: 2 })
        ));
        assert!(warrants.is_empty());
    }

    #[test]
    fn empty_responses_returns_no_activity() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let responses = vec![
            make_response(&agent, ChainStatus::Empty),
            make_response(&agent, ChainStatus::Empty),
        ];
        let (outcome, warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::NoActivity)
        ));
        assert!(warrants.is_empty());
    }

    #[test]
    fn unanimous_agreement_returns_agreed() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let responses = vec![
            make_response(&agent, valid_head(10, hash.clone())),
            make_response(&agent, valid_head(10, hash.clone())),
        ];
        let (outcome, warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed {
                head_seq: 10,
                head_hash,
                ..
            } if head_hash == hash
        ));
        assert!(warrants.is_empty());
    }

    #[test]
    fn head_disagreement_returns_retry() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash_a = ::fixt::fixt!(ActionHash);
        let hash_b = ::fixt::fixt!(ActionHash);
        let responses = vec![
            make_response(&agent, valid_head(5, hash_a)),
            make_response(&agent, valid_head(5, hash_b)),
        ];
        let (outcome, _warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::HeadDisagreement)
        ));
    }

    #[test]
    fn forked_status_returns_head_disagreement() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let fork = ChainStatus::Forked(holochain_zome_types::query::ChainFork {
            fork_seq: 3,
            first_action: hash.clone(),
            second_action: ::fixt::fixt!(ActionHash),
        });
        let responses = vec![
            make_response(&agent, valid_head(5, hash)),
            make_response(&agent, fork),
        ];
        let (outcome, _warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::HeadDisagreement)
        ));
    }

    #[test]
    fn warrants_naming_agent_are_returned_alongside_agreed() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let warrant = make_signed_warrant(&agent);
        let responses = vec![
            make_response(&agent, valid_head(5, hash.clone())),
            make_response_with_warrants(&agent, valid_head(5, hash), vec![warrant.clone()]),
        ];
        let (outcome, warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed { head_seq: 5, .. }
        ));
        assert_eq!(warrants.len(), 1);
    }

    #[test]
    fn warrants_naming_agent_are_returned_alongside_a_retry() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash_a = ::fixt::fixt!(ActionHash);
        let hash_b = ::fixt::fixt!(ActionHash);
        let warrant = make_signed_warrant(&agent);
        let responses = vec![
            make_response(&agent, valid_head(5, hash_a)),
            make_response_with_warrants(&agent, valid_head(5, hash_b), vec![warrant]),
        ];
        let (outcome, warrants) = evaluate_responses(&agent, responses, 2);
        // The peers still disagree on the head, so this round retries, but the warrant is
        // returned anyway so the caller can validate it and detect a permanent failure even
        // though this round produced no usable head.
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::HeadDisagreement)
        ));
        assert_eq!(warrants.len(), 1);
    }

    #[test]
    fn warrants_for_other_agent_are_ignored() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let other_agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        // Warrant is for `other_agent`, not `agent` so this should not be returned.
        let warrant = make_signed_warrant(&other_agent);
        let responses = vec![
            make_response(&agent, valid_head(5, hash.clone())),
            make_response_with_warrants(&agent, valid_head(5, hash), vec![warrant]),
        ];
        let (outcome, warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed { head_seq: 5, .. }
        ));
        assert!(warrants.is_empty());
    }

    #[test]
    fn invalid_status_uses_chain_head() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let head = ChainStatus::Invalid(ChainHead {
            action_seq: 5,
            hash: hash.clone(),
        });
        let responses = vec![
            make_response(&agent, head.clone()),
            make_response(&agent, head),
        ];
        let (outcome, _warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed { head_seq: 5, .. }
        ));
    }

    #[test]
    fn closed_status_uses_chain_head() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let head = ChainStatus::Closed(ChainHead {
            action_seq: 5,
            hash: hash.clone(),
        });
        let responses = vec![
            make_response(&agent, head.clone()),
            make_response(&agent, head),
        ];
        let (outcome, _warrants) = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed { head_seq: 5, .. }
        ));
    }

    #[test]
    fn records_from_correct_author_are_collected() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let record = make_record_for_agent(&agent);
        let responses = vec![make_response_with_records(
            &agent,
            valid_head(1, hash),
            vec![record],
        )];
        let (outcome, _warrants) = evaluate_responses(&agent, responses, 1);
        let AcquireOutcome::Agreed { records, .. } = outcome else {
            panic!("expected Agreed");
        };
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn records_from_wrong_author_are_excluded() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let other = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let record = make_record_for_agent(&other);
        let responses = vec![make_response_with_records(
            &agent,
            valid_head(1, hash),
            vec![record],
        )];
        let (outcome, _warrants) = evaluate_responses(&agent, responses, 1);
        let AcquireOutcome::Agreed { records, .. } = outcome else {
            panic!("expected Agreed");
        };
        assert!(records.is_empty());
    }

    #[tokio::test]
    async fn filter_wrong_author_is_discarded() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let other = ::fixt::fixt!(AgentPubKey);
        let record = make_record_for_agent(&other);
        let result = filter_records_by_author_and_signature(&agent, vec![record]).await;
        assert!(result.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn filter_valid_signature_is_kept() {
        let keystore = holochain_keystore::test_keystore();
        let agent = AgentPubKey::new_random(&keystore).await.unwrap();
        use ::fixt::prelude::*;
        let action = Action {
            header: ActionHeader {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(0),
                action_seq: 0,
                prev_action: None,
            },
            data: ActionData::Dna(DnaData {
                dna_hash: fixt!(DnaHash),
            }),
        };
        let sig = agent.sign(&keystore, action.clone()).await.unwrap();
        let signed =
            SignedActionHashed::with_presigned(ActionHashed::from_content_sync(action), sig);
        let record = Record::new(signed, RecordEntry::NA);
        let result = filter_records_by_author_and_signature(&agent, vec![record]).await;
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn filter_bad_signature_is_discarded() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let record = make_record_for_agent(&agent);
        let result = filter_records_by_author_and_signature(&agent, vec![record]).await;
        assert!(result.is_empty());
    }

    async fn cascade_with_network(network: holochain_p2p::DynHolochainP2pDna) -> CascadeImpl {
        let dht_id = holochain_state::data::Dht::new(std::sync::Arc::new(
            holo_hash::DnaHash::from_raw_36(vec![0u8; 36]),
        ));
        let dht_store = holochain_state::dht_store::DhtStore::new_test(dht_id)
            .await
            .unwrap();
        CascadeImpl::empty(dht_store).with_network(network)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn acquire_responses_merges_multiple_peer_responses() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let peer_a = ::fixt::fixt!(AgentPubKey);
        let peer_b = ::fixt::fixt!(AgentPubKey);
        let mut mock = holochain_p2p::MockHolochainP2pDnaT::new();
        let response_agent = agent.clone();
        mock.expect_get_agent_activity_multi()
            .returning(move |_, _, _| {
                Ok(vec![
                    (
                        peer_a.clone(),
                        make_response(&response_agent, valid_head(5, hash.clone())),
                    ),
                    (
                        peer_b.clone(),
                        make_response(&response_agent, valid_head(5, hash.clone())),
                    ),
                ])
            });
        let network: holochain_p2p::DynHolochainP2pDna = std::sync::Arc::new(mock);
        let cascade = cascade_with_network(network).await;

        let (outcome, warrants) = acquire_responses(&cascade, &agent, 2).await.unwrap();
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed { head_seq: 5, .. }
        ));
        assert!(warrants.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn acquire_responses_maps_insufficient_responses_to_retry() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let mut mock = holochain_p2p::MockHolochainP2pDnaT::new();
        mock.expect_get_agent_activity_multi().returning(|_, _, _| {
            Err(holochain_p2p::HolochainP2pError::InsufficientResponses {
                operation: "get_agent_activity_multi".into(),
                received: 1,
                required: 2,
            })
        });
        let network: holochain_p2p::DynHolochainP2pDna = std::sync::Arc::new(mock);
        let cascade = cascade_with_network(network).await;

        let (outcome, warrants) = acquire_responses(&cascade, &agent, 2).await.unwrap();
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::TooFewResponses { got: 1, need: 2 })
        ));
        assert!(warrants.is_empty());
    }
}
