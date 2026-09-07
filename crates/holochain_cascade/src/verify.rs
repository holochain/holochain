//! Verification applied to fetched get responses before they are cached:
//! per-op warrant pairing for rejected records, and signature checks on
//! rendered ops, warrants, and agent activity.

use holochain_keystore::AgentPubKeyExt;
use holochain_state::prelude::*;
use holochain_zome_types::warrant::{ChainIntegrityWarrant, SignedWarrant, WarrantProof};

/// Whether a rendered get response carries a `Rejected` record that is not
/// justified by an accompanying warrant. Such a response is dropped up front so
/// a malicious peer cannot serve a rejection it cannot prove: every rejected op
/// must be paired with a warrant against *that* op — an `InvalidChainOp` naming
/// the op's action, or a `ChainFork` against the op's author. A single unrelated
/// warrant is not enough.
pub(crate) fn rejected_without_warrant(rendered: &RenderedOps, warrants: &[SignedWarrant]) -> bool {
    rendered
        .ops
        .iter()
        .filter(|op| op.validation_status == Some(ValidationStatus::Rejected))
        .any(|op| !rejected_op_has_warrant(op, warrants))
}

/// Whether `warrants` contains one that justifies the rejection of `op`: an
/// `InvalidChainOp` naming the op's action, or a `ChainFork` against the op's
/// author (a forked chain invalidates every op the author put on it).
fn rejected_op_has_warrant(op: &RenderedOp, warrants: &[SignedWarrant]) -> bool {
    let action_hash = op.action.as_hash();
    let author = op.action.action().author();
    warrants.iter().any(|sw| {
        let WarrantProof::ChainIntegrity(w) = &sw.proof;
        match w {
            ChainIntegrityWarrant::InvalidChainOp { action, .. } => &action.0 == action_hash,
            ChainIntegrityWarrant::ChainFork { chain_author, .. } => chain_author == author,
        }
    })
}

/// Verify the action signatures (and warrant signature, if present) on every
/// `RenderedOps` in the batch. Batches where any signature fails verification
/// are logged at warn and dropped.
pub(crate) async fn verify_rendered_ops_batch(rendered_all: Vec<RenderedOps>) -> Vec<RenderedOps> {
    let mut verified = Vec::with_capacity(rendered_all.len());
    for rendered in rendered_all {
        if verify_rendered_ops_signatures(&rendered).await {
            verified.push(rendered);
        }
    }
    verified
}

async fn verify_rendered_ops_signatures(rendered: &RenderedOps) -> bool {
    for op in &rendered.ops {
        // Verify over the signed action — the same bytes the action was signed
        // over.
        let sa = &op.action;
        let action = &sa.hashed.content;
        match action
            .author()
            .verify_signature(sa.signature(), action)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    author = ?action.author(),
                    "Rendered op signature failed verification; dropping batch"
                );
                return false;
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "Error verifying rendered op signature; dropping batch"
                );
                return false;
            }
        }
    }

    if let Some(warrant_op) = &rendered.warrant {
        match warrant_op
            .author
            .verify_signature(warrant_op.signature(), warrant_op.warrant().clone())
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    author = ?warrant_op.author,
                    "Rendered warrant signature failed verification; dropping batch"
                );
                return false;
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "Error verifying rendered warrant signature; dropping batch"
                );
                return false;
            }
        }
    }

    true
}

/// Verify each agent-activity record and warrant in a
/// `MustGetAgentActivityResponse::Activity`. Records or warrants with bad
/// signatures are logged at warn and dropped.
pub(crate) async fn verify_activity_signatures(
    activity: Vec<AgentActivity>,
    warrants: Vec<WarrantOp>,
) -> (Vec<AgentActivity>, Vec<WarrantOp>) {
    let mut verified_activity = Vec::with_capacity(activity.len());
    for ra in activity {
        // Verify over the signed action — the same bytes it was signed over.
        let action = &ra.action.hashed.content;
        let verified = action
            .author()
            .verify_signature(ra.action.signature(), action)
            .await;
        match verified {
            Ok(true) => verified_activity.push(ra),
            Ok(false) => {
                tracing::warn!(
                    author = ?ra.action.hashed.content.author(),
                    "Activity record signature failed verification; dropping"
                );
            }
            Err(err) => {
                tracing::warn!(?err, "Error verifying activity record signature; dropping");
            }
        }
    }

    (verified_activity, verify_warrant_ops(warrants).await)
}

/// Whether `signature` is a signature over `warrant` by the warrant's author.
///
/// A warrant is hearsay until this holds: it accuses another agent, so a peer
/// that could serve one it did not sign could get any agent warranted.
async fn warrant_signature_is_valid(warrant: &Warrant, signature: &Signature) -> bool {
    match warrant
        .author
        .verify_signature(signature, warrant.clone())
        .await
    {
        Ok(valid) => valid,
        Err(err) => {
            tracing::warn!(?err, "Error verifying warrant signature; dropping");
            false
        }
    }
}

/// Drop the warrant ops whose signature is not by the warrant's author.
pub(crate) async fn verify_warrant_ops(warrants: Vec<WarrantOp>) -> Vec<WarrantOp> {
    let mut verified = Vec::with_capacity(warrants.len());
    for warrant_op in warrants {
        if warrant_signature_is_valid(warrant_op.warrant(), warrant_op.signature()).await {
            verified.push(warrant_op);
        } else {
            tracing::warn!(
                author = ?warrant_op.author,
                "Warrant signature failed verification; dropping"
            );
        }
    }
    verified
}

/// Drop the signed warrants whose signature is not by the warrant's author.
pub(crate) async fn verify_signed_warrants(warrants: Vec<SignedWarrant>) -> Vec<SignedWarrant> {
    let mut verified = Vec::with_capacity(warrants.len());
    for warrant in warrants {
        if warrant_signature_is_valid(warrant.data(), warrant.signature()).await {
            verified.push(warrant);
        } else {
            tracing::warn!(
                author = ?warrant.data().author,
                "Warrant signature failed verification; dropping"
            );
        }
    }
    verified
}

/// Drop everything in an agent-activity response that a peer cannot prove it
/// was given: records whose action signature is not by the action's author, and
/// warrants not signed by the warrant author.
///
/// Unlike the `must_get_agent_activity` path, this response is returned to the
/// caller rather than written to the `DhtStore`, so without this a peer could
/// hand a zome call actions the named agent never authored. `ChainItems::Hashes`
/// carries no signature to check and is passed through; the records it names are
/// themselves verified when they are fetched.
pub(crate) async fn verify_agent_activity_response(
    response: AgentActivityResponse,
) -> AgentActivityResponse {
    AgentActivityResponse {
        agent: response.agent,
        valid_activity: verify_chain_items(response.valid_activity).await,
        rejected_activity: verify_chain_items(response.rejected_activity).await,
        status: response.status,
        highest_observed: response.highest_observed,
        warrants: verify_signed_warrants(response.warrants).await,
    }
}

async fn verify_chain_items(items: ChainItems) -> ChainItems {
    let ChainItems::Full(records) = items else {
        return items;
    };
    let mut verified = Vec::with_capacity(records.len());
    for record in records {
        let action = record.action();
        match action
            .author()
            .verify_signature(record.signature(), action)
            .await
        {
            Ok(true) => verified.push(record),
            Ok(false) => {
                tracing::warn!(
                    author = ?action.author(),
                    "Agent activity record signature failed verification; dropping"
                );
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "Error verifying agent activity record signature; dropping"
                );
            }
        }
    }
    ChainItems::Full(verified)
}

#[cfg(test)]
mod rejected_warrant_invariant_tests {
    use super::*;
    use ::fixt::fixt;
    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator};
    use holochain_zome_types::warrant::{
        ChainIntegrityWarrant, SignedWarrant, Warrant, WarrantProof,
    };

    fn rendered_record(status: ValidationStatus) -> RenderedOps {
        let op = RenderedOp::new(
            fixt!(Action),
            fixt!(Signature),
            Some(status),
            ChainOpType::CreateRecord,
        )
        .unwrap();
        RenderedOps {
            entry: None,
            ops: vec![op],
            warrant: None,
        }
    }

    fn signed(proof: WarrantProof, warrantee: AgentPubKey) -> SignedWarrant {
        let warrant = Warrant::new(
            proof,
            fixt!(AgentPubKey),
            Timestamp::from_micros(0),
            warrantee,
        );
        SignedWarrant::new(warrant, fixt!(Signature))
    }

    /// An `InvalidChainOp` warrant naming a specific action.
    fn invalid_chain_op_warrant(action_hash: ActionHash, author: AgentPubKey) -> SignedWarrant {
        signed(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: author.clone(),
                action: (action_hash, fixt!(Signature)),
                chain_op_type: ChainOpType::CreateRecord,
                reason: "test".to_string(),
            }),
            author,
        )
    }

    /// A `ChainFork` warrant against a chain author.
    fn chain_fork_warrant(chain_author: AgentPubKey) -> SignedWarrant {
        signed(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
                chain_author: chain_author.clone(),
                action_pair: (
                    (fixt!(ActionHash), fixt!(Signature)),
                    (fixt!(ActionHash), fixt!(Signature)),
                ),
                seq: 0,
            }),
            chain_author,
        )
    }

    #[test]
    fn valid_record_needs_no_warrant() {
        assert!(!rejected_without_warrant(
            &rendered_record(ValidationStatus::Valid),
            &[]
        ));
    }

    #[test]
    fn rejected_record_without_warrant_is_rejected() {
        assert!(rejected_without_warrant(
            &rendered_record(ValidationStatus::Rejected),
            &[]
        ));
    }

    #[test]
    fn rejected_record_with_matching_invalid_chain_op_warrant_is_accepted() {
        let rendered = rendered_record(ValidationStatus::Rejected);
        let action_hash = rendered.ops[0].action.as_hash().clone();
        let author = rendered.ops[0].action.action().author().clone();
        assert!(!rejected_without_warrant(
            &rendered,
            &[invalid_chain_op_warrant(action_hash, author)]
        ));
    }

    #[test]
    fn rejected_record_with_chain_fork_against_author_is_accepted() {
        let rendered = rendered_record(ValidationStatus::Rejected);
        let author = rendered.ops[0].action.action().author().clone();
        assert!(!rejected_without_warrant(
            &rendered,
            &[chain_fork_warrant(author)]
        ));
    }

    #[test]
    fn rejected_record_with_unrelated_warrant_is_rejected() {
        // A warrant naming some other action/author does not justify this
        // rejected op, so the response is still dropped.
        assert!(rejected_without_warrant(
            &rendered_record(ValidationStatus::Rejected),
            &[invalid_chain_op_warrant(
                fixt!(ActionHash),
                fixt!(AgentPubKey)
            )]
        ));
    }
}

#[cfg(test)]
mod signature_verification_tests {
    use super::*;
    use holo_hash::{AgentPubKey, HoloHashed};
    use holochain_keystore::{test_keystore, MetaLairClient};

    /// A `CloseChain` naming `target` as its agent migration target, authored
    /// by `author`. Signing it with `target`'s key rather than `author`'s is
    /// the forgery that let a third party fork another agent's chain (#5981).
    fn close_chain(author: &AgentPubKey, target: &AgentPubKey) -> Action {
        Action {
            header: ActionHeader {
                author: author.clone(),
                timestamp: Timestamp::from_micros(42),
                action_seq: 5,
                prev_action: Some(ActionHash::from_raw_36(vec![1u8; 36])),
            },
            data: ActionData::CloseChain(CloseChainData {
                new_target: Some(MigrationTarget::Agent(target.clone())),
            }),
        }
    }

    async fn signed_by(
        keystore: &MetaLairClient,
        signer: &AgentPubKey,
        action: &Action,
    ) -> Signature {
        signer.sign(keystore, action).await.unwrap()
    }

    fn rendered(action: Action, signature: Signature) -> RenderedOps {
        RenderedOps {
            entry: None,
            ops: vec![RenderedOp::new(
                action,
                signature,
                Some(ValidationStatus::Valid),
                ChainOpType::AgentActivity,
            )
            .unwrap()],
            warrant: None,
        }
    }

    fn response(valid_activity: ChainItems) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: AgentPubKey::from_raw_36(vec![2u8; 36]),
            valid_activity,
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            highest_observed: None,
            warrants: Vec::new(),
        }
    }

    fn activity(action: Action, signature: Signature) -> AgentActivity {
        AgentActivity {
            action: SignedActionHashed::with_presigned(
                HoloHashed::from_content_sync(action),
                signature,
            ),
            cached_entry: None,
        }
    }

    // `test_keystore()` spawns lair onto a blocking task, which requires a multi-threaded runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn rendered_ops_signed_by_the_migration_target_are_dropped() {
        let keystore = test_keystore();
        let author = AgentPubKey::new_random(&keystore).await.unwrap();
        let target = AgentPubKey::new_random(&keystore).await.unwrap();
        let action = close_chain(&author, &target);

        let by_author = signed_by(&keystore, &author, &action).await;
        assert_eq!(
            verify_rendered_ops_batch(vec![rendered(action.clone(), by_author)])
                .await
                .len(),
            1,
            "an op signed by its author must be kept"
        );

        let by_target = signed_by(&keystore, &target, &action).await;
        assert!(
            verify_rendered_ops_batch(vec![rendered(action, by_target)])
                .await
                .is_empty(),
            "an op signed by the migration target rather than the author must be dropped"
        );
    }

    /// A get response is returned to the caller rather than written to the
    /// `DhtStore`, so signatures on the records it carries have to be checked
    /// here or nothing checks them at all.
    // `test_keystore()` spawns lair onto a blocking task, which requires a multi-threaded runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn agent_activity_records_not_signed_by_their_author_are_dropped() {
        let keystore = test_keystore();
        let author = AgentPubKey::new_random(&keystore).await.unwrap();
        let peer = AgentPubKey::new_random(&keystore).await.unwrap();
        let action = close_chain(&author, &peer);

        let record = |signature| {
            Record::new(
                SignedActionHashed::with_presigned(
                    HoloHashed::from_content_sync(action.clone()),
                    signature,
                ),
                RecordEntry::or_not_applicable(None),
            )
        };
        let by_author = record(signed_by(&keystore, &author, &action).await);
        let by_peer = record(signed_by(&keystore, &peer, &action).await);

        let verified = verify_agent_activity_response(response(ChainItems::Full(vec![
            by_author.clone(),
            by_peer,
        ])))
        .await;

        assert_eq!(
            verified.valid_activity,
            ChainItems::Full(vec![by_author]),
            "only the record signed by its author survives"
        );
    }

    /// `ChainItems::Hashes` carries no signature to check, so it must pass
    /// through rather than be dropped as unverifiable.
    #[tokio::test(flavor = "multi_thread")]
    async fn agent_activity_hashes_pass_through() {
        let hashes = ChainItems::Hashes(vec![(0, ActionHash::from_raw_36(vec![3u8; 36]))]);
        let verified = verify_agent_activity_response(response(hashes.clone())).await;
        assert_eq!(verified.valid_activity, hashes);
    }

    /// A warrant accuses another agent, so a peer that did not sign it must not
    /// have it staged for validation or added to the scratch on its say-so.
    // `test_keystore()` spawns lair onto a blocking task, which requires a multi-threaded runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn warrants_not_signed_by_their_author_are_dropped() {
        let keystore = test_keystore();
        let warrant_author = AgentPubKey::new_random(&keystore).await.unwrap();
        let liar = AgentPubKey::new_random(&keystore).await.unwrap();
        let warrant = Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
                chain_author: warrant_author.clone(),
                action_pair: (
                    (ActionHash::from_raw_36(vec![4u8; 36]), Signature([0; 64])),
                    (ActionHash::from_raw_36(vec![5u8; 36]), Signature([0; 64])),
                ),
                seq: 0,
            }),
            warrant_author.clone(),
            Timestamp::from_micros(42),
            AgentPubKey::from_raw_36(vec![6u8; 36]),
        );

        let honest = SignedWarrant::new(
            warrant.clone(),
            warrant_author
                .sign(&keystore, warrant.clone())
                .await
                .unwrap(),
        );
        let forged = SignedWarrant::new(
            warrant.clone(),
            liar.sign(&keystore, warrant.clone()).await.unwrap(),
        );

        assert_eq!(
            verify_signed_warrants(vec![honest.clone(), forged.clone()]).await,
            vec![honest.clone()],
            "a warrant not signed by its own author must be dropped"
        );
        assert_eq!(
            verify_warrant_ops(vec![
                WarrantOp::from(honest.clone()),
                WarrantOp::from(forged)
            ])
            .await,
            vec![WarrantOp::from(honest)],
            "the same rule applies to warrant ops staged for validation"
        );
    }

    // `test_keystore()` spawns lair onto a blocking task, which requires a multi-threaded runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn activity_signed_by_the_migration_target_is_dropped() {
        let keystore = test_keystore();
        let author = AgentPubKey::new_random(&keystore).await.unwrap();
        let target = AgentPubKey::new_random(&keystore).await.unwrap();
        let action = close_chain(&author, &target);

        let by_author = signed_by(&keystore, &author, &action).await;
        let (kept, _) =
            verify_activity_signatures(vec![activity(action.clone(), by_author)], vec![]).await;
        assert_eq!(kept.len(), 1, "a record signed by its author must be kept");

        let by_target = signed_by(&keystore, &target, &action).await;
        let (kept, _) = verify_activity_signatures(vec![activity(action, by_target)], vec![]).await;
        assert!(
            kept.is_empty(),
            "a record signed by the migration target rather than the author must be dropped"
        );
    }
}
