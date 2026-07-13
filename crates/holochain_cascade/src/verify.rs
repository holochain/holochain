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
            .signer()
            .verify_signature(sa.signature(), action)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    signer = ?action.signer(),
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
    activity: Vec<RegisterAgentActivity>,
    warrants: Vec<WarrantOp>,
) -> (Vec<RegisterAgentActivity>, Vec<WarrantOp>) {
    let mut verified_activity = Vec::with_capacity(activity.len());
    for ra in activity {
        // Verify over the signed action — the same bytes it was signed over.
        let action = &ra.action.hashed.content;
        let verified = action
            .signer()
            .verify_signature(ra.action.signature(), action)
            .await;
        match verified {
            Ok(true) => verified_activity.push(ra),
            Ok(false) => {
                tracing::warn!(
                    signer = ?ra.action.hashed.content.signer(),
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
            ChainOpType::StoreRecord,
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
                chain_op_type: ChainOpType::StoreRecord,
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
