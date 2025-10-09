use super::*;
use crate::fixt::ActionHashFixturator;
use ::fixt::fixt;

/// Helper function to create a RegisterAgentActivity
pub fn create_activity(seq: u32) -> RegisterAgentActivity {
    let mut create = fixt!(Create);
    create.action_seq = seq;

    RegisterAgentActivity {
        action: SignedHashed::new_unchecked(Action::Create(create.clone()), fixt!(Signature)),
        cached_entry: None,
    }
}

/// Helper function to create a WarrantOp
pub fn create_warrant_op() -> WarrantOp {
    let author = fake_agent_pub_key(0);
    Signed::new(
        Warrant::new_now(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: author.clone(),
                action: (fixt!(ActionHash), fixt!(Signature)),
                chain_op_type: ChainOpType::RegisterAddLink,
            }),
            fake_agent_pub_key(1),
            author,
        ),
        fixt!(Signature),
    )
    .into()
}

/// Helper function to create a RegisterAgentActivity with specific hash and prev_action
pub fn create_activity_with_prev(
    seq: u32,
    hash: ActionHash,
    prev: ActionHash,
) -> RegisterAgentActivity {
    let mut create = fixt!(Create);
    create.action_seq = seq;
    create.prev_action = prev;

    RegisterAgentActivity {
        action: SignedActionHashed::with_presigned(
            ActionHashed::with_pre_hashed(Action::Create(create), hash),
            fixt!(Signature),
        ),
        cached_entry: None,
    }
}
