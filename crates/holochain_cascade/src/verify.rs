//! Verification applied to fetched get responses before they are cached:
//! per-op warrant pairing for rejected records, entry hash checks on rendered
//! ops, and signature checks on rendered ops, warrants, and agent activity.

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

/// Whether an op in `rendered` that stores the served entry names a different
/// entry hash in its action than the served entry actually hashes to.
///
/// `rendered.entry` is hashed from the served bytes when the response is
/// rendered, so its hash is the true hash of what the peer sent, while the
/// action's entry hash is what the author committed to. The entry is not
/// covered by the action signature, so a peer can pair a genuine action with
/// fabricated entry bytes; such a response must not reach the store.
///
/// Only the ops whose actions own the served entry are compared:
/// `CreateEntry` for an entry response and `CreateRecord` for a record
/// response. Update and delete ops served alongside them refer to the original
/// entry or record but may name a different entry in their own action. An
/// owning op whose action has no entry hash cannot be paired with an entry, so
/// it counts as a mismatch. An entry without an owning op is also a mismatch.
/// A response without an entry has nothing to compare.
pub(crate) fn entry_hash_mismatch(rendered: &RenderedOps) -> bool {
    let Some(entry) = rendered.entry.as_ref() else {
        return false;
    };
    let mut owning_ops = rendered.ops.iter().filter(|op| {
        matches!(
            op.op_type,
            ChainOpType::CreateEntry | ChainOpType::CreateRecord
        )
    });
    let Some(first_owner) = owning_ops.next() else {
        return true;
    };
    first_owner.action.action().entry_hash() != Some(entry.as_hash())
        || owning_ops.any(|op| op.action.action().entry_hash() != Some(entry.as_hash()))
}

/// Verify the entry hash and the action signatures (and warrant signature, if
/// present) on every `RenderedOps` in the batch.
///
/// Batches where the served entry does not match the entry hash named by an op
/// that stores it, or where any signature fails verification, are logged at
/// warn and dropped. The entry check runs first: it is synchronous and cheaper
/// than signature verification.
pub(crate) async fn verify_rendered_ops_batch(rendered_all: Vec<RenderedOps>) -> Vec<RenderedOps> {
    let mut verified = Vec::with_capacity(rendered_all.len());
    for rendered in rendered_all {
        if entry_hash_mismatch(&rendered) {
            tracing::warn!(
                "Rendered entry does not hash to the entry hash named by its action; dropping batch"
            );
            continue;
        }
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

    let mut verified_warrants = Vec::with_capacity(warrants.len());
    for warrant_op in warrants {
        match warrant_op
            .author
            .verify_signature(warrant_op.signature(), warrant_op.warrant().clone())
            .await
        {
            Ok(true) => verified_warrants.push(warrant_op),
            Ok(false) => {
                tracing::warn!(
                    author = ?warrant_op.author,
                    "Activity warrant signature failed verification; dropping"
                );
            }
            Err(err) => {
                tracing::warn!(?err, "Error verifying activity warrant signature; dropping");
            }
        }
    }

    (verified_activity, verified_warrants)
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

#[cfg(test)]
mod entry_hash_tests {
    use super::*;
    use ::fixt::fixt;
    use holo_hash::AgentPubKey;
    use holochain_keystore::test_keystore;
    use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
    use holochain_zome_types::fixt::{
        ActionFixturator, CreateAction, CreateLinkAction, UpdateAction,
    };

    /// A fixed public app entry, hashed from its own bytes the way `render`
    /// hashes a served entry.
    fn served_entry() -> EntryHashed {
        EntryHashed::from_content_sync(Entry::App(AppEntryBytes(SerializedBytes::from(
            UnsafeBytes::from(vec![1, 3, 5]),
        ))))
    }

    /// A create action naming `entry_hash` as its entry.
    fn create_naming(entry_hash: EntryHash) -> Action {
        let mut action = fixt!(Action, CreateAction);
        *action.entry_hash_mut().unwrap() = entry_hash;
        action
    }

    /// An update action naming `entry_hash` as its new entry.
    fn update_naming(entry_hash: EntryHash) -> Action {
        let mut action = fixt!(Action, UpdateAction);
        *action.entry_hash_mut().unwrap() = entry_hash;
        action
    }

    fn rendered(entry: Option<EntryHashed>, ops: Vec<(Action, ChainOpType)>) -> RenderedOps {
        RenderedOps {
            entry,
            ops: ops
                .into_iter()
                .map(|(action, op_type)| {
                    RenderedOp::new(
                        action,
                        fixt!(Signature),
                        Some(ValidationStatus::Valid),
                        op_type,
                    )
                    .unwrap()
                })
                .collect(),
            warrant: None,
        }
    }

    #[test]
    fn entry_hash_mismatch_cases() {
        let entry = served_entry();
        let entry_hash = entry.as_hash().clone();
        let other_entry = EntryHashed::from_content_sync(Entry::App(AppEntryBytes(
            SerializedBytes::from(UnsafeBytes::from(vec![2, 4, 6])),
        )));
        let other_entry_hash = other_entry.as_hash().clone();
        assert_ne!(entry_hash, other_entry_hash);

        let create_link = fixt!(Action, CreateLinkAction);
        assert!(create_link.entry_hash().is_none());

        let cases = [
            (
                "entry matching its create action passes",
                rendered(
                    Some(entry.clone()),
                    vec![(create_naming(entry_hash.clone()), ChainOpType::CreateEntry)],
                ),
                false,
            ),
            (
                "entry not matching its create action is a mismatch",
                rendered(
                    Some(entry.clone()),
                    vec![(
                        create_naming(other_entry_hash.clone()),
                        ChainOpType::CreateEntry,
                    )],
                ),
                true,
            ),
            (
                "update entry naming a replacement is not compared",
                rendered(
                    Some(entry.clone()),
                    vec![
                        (create_naming(entry_hash.clone()), ChainOpType::CreateEntry),
                        (
                            update_naming(other_entry_hash.clone()),
                            ChainOpType::UpdateEntry,
                        ),
                    ],
                ),
                false,
            ),
            (
                "entry without any ops is a mismatch",
                rendered(Some(entry.clone()), vec![]),
                true,
            ),
            (
                "entry with only an update op is a mismatch",
                rendered(
                    Some(entry.clone()),
                    vec![(
                        update_naming(other_entry_hash.clone()),
                        ChainOpType::UpdateEntry,
                    )],
                ),
                true,
            ),
            (
                "create record not matching is a mismatch",
                rendered(
                    Some(entry.clone()),
                    vec![(
                        create_naming(other_entry_hash.clone()),
                        ChainOpType::CreateRecord,
                    )],
                ),
                true,
            ),
            (
                "one bad op among good ones is a mismatch",
                rendered(
                    Some(entry.clone()),
                    vec![
                        (create_naming(entry_hash.clone()), ChainOpType::CreateEntry),
                        (
                            create_naming(other_entry_hash.clone()),
                            ChainOpType::CreateEntry,
                        ),
                    ],
                ),
                true,
            ),
            (
                "update record naming its own entry is not compared",
                rendered(
                    Some(entry.clone()),
                    vec![
                        (create_naming(entry_hash.clone()), ChainOpType::CreateRecord),
                        (
                            update_naming(other_entry_hash.clone()),
                            ChainOpType::UpdateRecord,
                        ),
                    ],
                ),
                false,
            ),
            (
                "record without an entry hash served with an entry is a mismatch",
                rendered(Some(entry), vec![(create_link, ChainOpType::CreateRecord)]),
                true,
            ),
            (
                "response without an entry passes",
                rendered(
                    None,
                    vec![(create_naming(other_entry_hash), ChainOpType::CreateEntry)],
                ),
                false,
            ),
        ];

        for (name, rendered, expected) in cases {
            assert_eq!(entry_hash_mismatch(&rendered), expected, "{name}");
        }
    }

    // `test_keystore()` spawns lair onto a blocking task, which requires a
    // multi-threaded runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn rendered_ops_with_a_fabricated_entry_are_dropped() {
        let keystore = test_keystore();
        let author = AgentPubKey::new_random(&keystore).await.unwrap();
        let entry = served_entry();

        let mut genuine = create_naming(entry.as_hash().clone());
        genuine.header.author = author.clone();
        let signature = author.sign(&keystore, &genuine).await.unwrap();
        let ops = vec![RenderedOp::new(
            genuine.clone(),
            signature,
            Some(ValidationStatus::Valid),
            ChainOpType::CreateEntry,
        )
        .unwrap()];
        let kept = verify_rendered_ops_batch(vec![RenderedOps {
            entry: Some(entry.clone()),
            ops: ops.clone(),
            warrant: None,
        }])
        .await;
        assert_eq!(
            kept.len(),
            1,
            "a signed op served with its own entry is kept"
        );

        // Same signed action, served with different entry bytes.
        let other_entry = EntryHashed::from_content_sync(Entry::App(AppEntryBytes(
            SerializedBytes::from(UnsafeBytes::from(vec![2, 4, 6])),
        )));
        assert_ne!(other_entry.as_hash(), entry.as_hash());
        let kept = verify_rendered_ops_batch(vec![RenderedOps {
            entry: Some(other_entry),
            ops,
            warrant: None,
        }])
        .await;
        assert!(
            kept.is_empty(),
            "a signed op served with fabricated entry bytes must be dropped"
        );
    }
}
