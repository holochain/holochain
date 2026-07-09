//! Part of the restore workflow. Acquires the agent's chain from the DHT and pins the target chain
//! head by requiring unanimous agreement across responses.
//!
//! ## Thin-wrapper note
//!
//! [`acquire_responses`] currently calls the existing [`CascadeImpl::get_agent_activity`] function
//! and wraps the merged results in a singleton [`Vec`].
//! When [#5799](https://github.com/holochain/holochain/issues/5799) is completed, the new
//! `get_agent_activity_multi` function will replace it, which will return one response per peer,
//! enabling genuine multi-peer agreement. The evaluation logic in [`evaluate_responses`] is already
//! written for that function's response and should remain unchanged.

use holo_hash::{ActionHash, AgentPubKey};
use holochain_cascade::CascadeImpl;
use holochain_keystore::AgentPubKeyExt;
use holochain_p2p::actor::GetActivityOptions;
use holochain_types::activity::{AgentActivityResponse, ChainItems};
use holochain_zome_types::{
    entry::GetOptions,
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
    /// One or more peer responses included [`SignedWarrant`]s for `agent`.
    /// These must be validated locally before chain reconstruction can proceed; a validated
    /// [`holochain_zome_types::warrant::ChainIntegrityWarrant`] is grounds for permanent failure
    /// while a rejected warrant means the chain is considered uncorrupted.
    WarrantsPending(Vec<SignedWarrant>),
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
    /// [`ChainStatus::Forked`] status without a validating warrant.
    HeadDisagreement,
    /// Every peer response reports [`ChainStatus::Empty`] meaning that either the agent's chain
    /// does not yet exist on the DHT or there is no peer connectivity.
    NoActivity,
}

/// Queries peers to get the agent's full chain activity from the DHT, evaluates the responses for
/// quorum and chain-head agreement to produce an [`AcquireOutcome`].
/// When agreement is reached it filters the collected records for those with a valid signature
/// against `agent`.
pub(super) async fn acquire_responses(
    cascade: &CascadeImpl,
    agent: &AgentPubKey,
    quorum: u8,
) -> WorkflowResult<AcquireOutcome> {
    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: true,
        include_warrants: true,
        include_full_records: true,
        get_options: GetOptions::network(),
        ..Default::default()
    };
    let query = ChainQueryFilter::new().include_entries(true);
    let response = cascade
        // TODO: Replace this with `get_agent_activity_multi` when #5799 is completed.
        .get_agent_activity(agent.clone(), query, options)
        .await
        .map_err(WorkflowError::CascadeError)?;

    let mut outcome = evaluate_responses(agent, vec![response], quorum);

    // If the peers agreed on a head, filter the collected records by signature.
    // Records whose signature fails verification are dropped so that a misbehaving peer cannot
    // abort the restore by serving forgeries.
    if let AcquireOutcome::Agreed { records, .. } = &mut outcome {
        *records = filter_records_by_author_and_signature(agent, std::mem::take(records)).await;
    }

    Ok(outcome)
}

/// Evaluates a set of [`AgentActivityResponse`]s and produces an [`AcquireOutcome`] by checking
/// quorum, surfacing warrants, requiring unanimous chain-head agreement, and collecting
/// the full records. No signature filtering is applied on the returned records so this should be
/// done by the caller.
pub(super) fn evaluate_responses(
    agent: &AgentPubKey,
    responses: Vec<AgentActivityResponse>,
    quorum: u8,
) -> AcquireOutcome {
    if responses.len() < quorum as usize {
        return AcquireOutcome::Retry(RetryReason::TooFewResponses {
            got: responses.len(),
            need: quorum,
        });
    }

    // Collect warrants naming this agent from every response.
    let warrants_for_agent: Vec<SignedWarrant> = responses
        .iter()
        .flat_map(|resp| resp.warrants.iter())
        .filter(|warrant| warrant.warrantee == *agent)
        .cloned()
        .collect();

    if !warrants_for_agent.is_empty() {
        return AcquireOutcome::WarrantsPending(warrants_for_agent);
    }

    // Determine the agreed chain head. Every non-empty response must share the same
    // (action_seq, hash) pair.
    let mut agreed_head = None;
    for response in &responses {
        match &response.status {
            ChainStatus::Empty => {}

            ChainStatus::Valid(head) | ChainStatus::Invalid(head) | ChainStatus::Closed(head) => {
                if let Some((prev_seq, prev_hash)) = &agreed_head {
                    if &head.action_seq != prev_seq || &head.hash != prev_hash {
                        return AcquireOutcome::Retry(RetryReason::HeadDisagreement);
                    }
                } else {
                    agreed_head = Some((head.action_seq, head.hash.clone()));
                }
            }

            // All valid warrants are handled above. Therefore, getting this forked status without
            // an accompanying warrant means that the peer cannot prove the fork and we should just
            // treat this as a disagreement, prompting the workflow to retry.
            ChainStatus::Forked(_) => return AcquireOutcome::Retry(RetryReason::HeadDisagreement),
        }
    }

    let Some((head_seq, head_hash)) = agreed_head else {
        return AcquireOutcome::Retry(RetryReason::NoActivity);
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

    AcquireOutcome::Agreed {
        head_seq,
        head_hash,
        records,
    }
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
        let action = Action::Dna(Dna {
            author: agent.clone(),
            timestamp: Timestamp::from_micros(0),
            hash: fixt!(DnaHash),
        });
        let action_hashed = ActionHashed::from_content_sync(action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        Record::new(signed, None)
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
        let outcome = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::TooFewResponses { got: 1, need: 2 })
        ));
    }

    #[test]
    fn empty_responses_returns_no_activity() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let responses = vec![
            make_response(&agent, ChainStatus::Empty),
            make_response(&agent, ChainStatus::Empty),
        ];
        let outcome = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::NoActivity)
        ));
    }

    #[test]
    fn unanimous_agreement_returns_agreed() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let responses = vec![
            make_response(&agent, valid_head(10, hash.clone())),
            make_response(&agent, valid_head(10, hash.clone())),
        ];
        let outcome = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed {
                head_seq: 10,
                head_hash,
                ..
            } if head_hash == hash
        ));
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
        let outcome = evaluate_responses(&agent, responses, 2);
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
        let outcome = evaluate_responses(&agent, responses, 2);
        assert!(matches!(
            outcome,
            AcquireOutcome::Retry(RetryReason::HeadDisagreement)
        ));
    }

    #[test]
    fn warrants_naming_agent_returns_pending() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        let warrant = make_signed_warrant(&agent);
        let responses = vec![
            make_response(&agent, valid_head(5, hash.clone())),
            make_response_with_warrants(&agent, valid_head(5, hash), vec![warrant.clone()]),
        ];
        let outcome = evaluate_responses(&agent, responses, 2);
        assert!(matches!(outcome,
        AcquireOutcome::WarrantsPending(warrants) if warrants.len() == 1
        ));
    }

    #[test]
    fn warrants_for_other_agent_are_ignored() {
        let agent = ::fixt::fixt!(AgentPubKey);
        let other_agent = ::fixt::fixt!(AgentPubKey);
        let hash = ::fixt::fixt!(ActionHash);
        // Warrant is for `other_agent`, not `agent` so this should not block the restore.
        let warrant = make_signed_warrant(&other_agent);
        let responses = vec![
            make_response(&agent, valid_head(5, hash.clone())),
            make_response_with_warrants(&agent, valid_head(5, hash), vec![warrant]),
        ];
        let outcome = evaluate_responses(&agent, responses, 2);
        // Should proceed as Agreed (warrant was for a different agent).
        assert!(matches!(
            outcome,
            AcquireOutcome::Agreed { head_seq: 5, .. }
        ));
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
        let outcome = evaluate_responses(&agent, responses, 2);
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
        let outcome = evaluate_responses(&agent, responses, 2);
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
        let outcome = evaluate_responses(&agent, responses, 1);
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
        let outcome = evaluate_responses(&agent, responses, 1);
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
        let action = Action::Dna(Dna {
            author: agent.clone(),
            timestamp: Timestamp::from_micros(0),
            hash: fixt!(DnaHash),
        });
        let sig = agent.sign(&keystore, action.clone()).await.unwrap();
        let signed =
            SignedActionHashed::with_presigned(ActionHashed::from_content_sync(action), sig);
        let record = Record::new(signed, None);
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
}
