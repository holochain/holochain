use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::core::workflow::sys_validation_workflow::validate_op;
use crate::core::MockDhtOpSender;
use crate::prelude::Action;
use crate::prelude::ActionHashed;
use crate::prelude::DhtOp;
use crate::prelude::DnaDef;
use crate::prelude::DnaDefHashed;
use crate::prelude::SignedActionHashed;
use fixt::prelude::*;
use futures::FutureExt;
use holochain_cascade::CascadeSource;
use holochain_cascade::MockCascade;
use holochain_keystore::AgentPubKeyExt;
use holochain_state::prelude::CreateFixturator;
use holochain_state::prelude::SignatureFixturator;
use holochain_types::prelude::SignedActionHashedExt;
use holochain_zome_types::record::Record;
use crate::prelude::AgentPubKeyFixturator;
use crate::prelude::Entry;
use crate::core::workflow::WorkflowResult;
use crate::prelude::AgentPubKey;

// A test can't be written for `dna_op_with_previous_action` because the types do not permit constructing this scenario.

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_op() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    let op = test_op(agent.clone().into());

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    let mut create_action = fixt!(Create);
    create_action.author = agent.clone().into();
    let action = Action::Create(create_action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    cascade.expect_retrieve_action().once().returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let agent = agent.clone();
            let keystore = keystore.clone();
            let signed_action = signed_action.clone();
            async move {
                Ok(Some((signed_action, CascadeSource::Local)))
            }
            .boxed()
        }
    });

    cascade.expect_retrieve().return_once(move |_hash, _options| {
        let signed_action = signed_action.clone();
        async move {
            Ok(Some((Record::new(signed_action, Some(Entry::Agent(fixt!(AgentPubKey)))), CascadeSource::Local)))
        }.boxed()
    });

    let validation_outcome = validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>)
        .await
        .unwrap();

    assert!(matches!(validation_outcome, Outcome::Accepted));
}

#[tokio::test(flavor = "multi_thread")]
async fn non_dna_op_as_first_action() {
    holochain_trace::test_run().unwrap();

    let mut create = fixt!(Create);
    create.action_seq = 0; // Not valid, a DNA should always be first
    let create_action = Action::Create(create);
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), create_action);

    let outcome = TestCase::new(op)
        .execute()
        .await
        .unwrap();

    assert!(matches!(outcome, Outcome::Rejected));
}

struct TestCase {
    op: DhtOp,
    cascade: MockCascade,
}

impl TestCase {
    fn new(op: DhtOp) -> Self {
        TestCase {
            op,
            cascade: MockCascade::new(),
        }
    }

    async fn execute(&self) -> WorkflowResult<Outcome> {
        let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
        let dna_def = DnaDefHashed::from_content_sync(dna_def);

        validate_op(&self.op, &dna_def, &self.cascade, None::<&MockDhtOpSender>)
            .await
    }
}

fn test_op(author: AgentPubKey) -> DhtOp {
    let mut create_action = fixt!(Create);
    create_action.author = author.into();
    create_action.action_seq = 10; // Should not be first
    let action = Action::Create(create_action);

    DhtOp::RegisterAgentActivity(fixt!(Signature), action)
}
