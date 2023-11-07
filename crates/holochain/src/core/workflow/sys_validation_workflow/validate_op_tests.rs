use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::core::workflow::sys_validation_workflow::validate_op;
use crate::core::workflow::WorkflowResult;
use crate::core::MockDhtOpSender;
use crate::prelude::Action;
use crate::prelude::ActionHashed;
use crate::prelude::AgentPubKeyFixturator;
use crate::prelude::AgentValidationPkgFixturator;
use crate::prelude::DhtOp;
use crate::prelude::DnaDef;
use crate::prelude::DnaDefHashed;
use crate::prelude::Entry;
use crate::prelude::HoloHashed;
use crate::prelude::SignedActionHashed;
use crate::prelude::Timestamp;
use fixt::prelude::*;
use futures::FutureExt;
use hdk::prelude::Dna as HdkDna;
use holochain_cascade::CascadeSource;
use holochain_cascade::MockCascade;
use holochain_state::prelude::CreateFixturator;
use holochain_state::prelude::SignatureFixturator;
use holochain_types::prelude::SignedActionHashedExt;
use holochain_zome_types::prelude::AgentValidationPkg;
use holochain_zome_types::record::Record;
use holochain_zome_types::record::SignedHashed;
use crate::prelude::DnaHashFixturator;

// A test can't be written for `dna_op_with_previous_action` because the types do not permit constructing this scenario.

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_dna_op() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    let mut test_case = TestCase::new();

    let dna_action = HdkDna {
        author: agent.clone().into(),
        timestamp: Timestamp::now().into(),
        hash: test_case.dna_def_hash().hash,
    };
    let action = Action::Dna(dna_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let outcome = test_case.with_op(op).execute().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_dna_op_mismatched_dna_hash() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    let dna_action = HdkDna {
        author: agent.clone().into(),
        timestamp: Timestamp::now().into(),
        hash: fixt!(DnaHash), // Will not match the space
    };
    let action = Action::Dna(dna_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let outcome = TestCase::new().with_op(op).execute().await.unwrap();

    // TODO this test assertion would be better if it was more specific
    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_dna_op_before_origin_time() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    let mut test_case = TestCase::new();

    test_case.dna_def_mut().modifiers.origin_time = (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap();

    let dna_action = HdkDna {
        author: agent.clone().into(),
        timestamp: Timestamp::now().into(),
        hash: test_case.dna_def_hash().hash,
    };
    let action = Action::Dna(dna_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let outcome = test_case.with_op(op).execute().await.unwrap();

    // TODO this test assertion would be better if it was more specific
    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_avp_op() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    let mut test_case = TestCase::new();

    let dna_action = HdkDna {
        author: agent.clone().into(),
        timestamp: Timestamp::now().into(),
        hash: test_case.dna_def_hash().hash,
    };
    let dna_action = Action::Dna(dna_action);
    let dna_action_hashed = ActionHashed::from_content_sync(dna_action);
    let dna_action_signed = SignedActionHashed::sign(&keystore, dna_action_hashed)
        .await
        .unwrap();

    let action = AgentValidationPkg {
        author: agent.clone().into(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: dna_action_signed.as_hash().clone(),
        membrane_proof: None,
    };
    let avp_action = Action::AgentValidationPkg(action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), avp_action);

    test_case
        .cascade_mut()
        .expect_retrieve_action()
        .once()
        .returning({
            let dna_action_signed = dna_action_signed.clone();
            move |_, _| {
                let agent = agent.clone();
                let keystore = keystore.clone();
                let dna_action_signed = dna_action_signed.clone();
                async move { Ok(Some((dna_action_signed, CascadeSource::Local))) }.boxed()
            }
        });

    test_case
        .cascade_mut()
        .expect_retrieve()
        .return_once(move |_hash, _options| {
            let dna_action_signed = dna_action_signed.clone();
            async move {
                Ok(Some((
                    Record::new(dna_action_signed, None),
                    CascadeSource::Local,
                )))
            }
            .boxed()
        });

    let outcome = test_case.with_op(op).execute().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_create_op() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = agent.clone().into();
    validation_package_action.action_seq = 10;
    let action = Action::AgentValidationPkg(validation_package_action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    // and current which needs values from previous
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let action = Action::Create(create_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().once().returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .return_once(move |_hash, _options| {
            let signed_action = signed_action.clone();
            async move {
                Ok(Some((
                    Record::new(signed_action, None),
                    CascadeSource::Local,
                )))
            }
            .boxed()
        });

    let validation_outcome = validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>)
        .await
        .unwrap();

    assert!(matches!(validation_outcome, Outcome::Accepted));
}

// TODO this hits code which claims to be unreachable. Clearly it isn't so investigate the code path.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "TODO fix this test"]
async fn crash_case() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut create_action = fixt!(AgentValidationPkg);
    create_action.author = agent.clone().into();
    create_action.timestamp = Timestamp::now().into();
    create_action.action_seq = 10;
    let action = Action::AgentValidationPkg(create_action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    // and current which needs values from previous
    let op = test_op(signed_action.clone());

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().once().returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let agent = agent.clone();
            let keystore = keystore.clone();
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .return_once(move |_hash, _options| {
            let signed_action = signed_action.clone();
            async move {
                // TODO this line createx the problem, expects a None value
                Ok(Some((
                    Record::new(signed_action, Some(Entry::Agent(fixt!(AgentPubKey)))),
                    CascadeSource::Local,
                )))
            }
            .boxed()
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

    let outcome = TestCase::new().with_op(op).execute().await.unwrap();

    assert!(matches!(outcome, Outcome::Rejected));
}

struct TestCase {
    op: Option<DhtOp>,
    cascade: MockCascade,
    dna_def: DnaDef,
}

impl TestCase {
    fn new() -> Self {
        let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);

        TestCase {
            op: None,
            cascade: MockCascade::new(),
            dna_def,
        }
    }

    pub fn with_op(&mut self, op: DhtOp) -> &mut Self {
        self.op = Some(op);
        self
    }

    pub fn cascade_mut(&mut self) -> &mut MockCascade {
        &mut self.cascade
    }

    pub fn dna_def_mut(&mut self) -> &mut DnaDef {
        &mut self.dna_def
    }

    pub fn dna_def_hash(&self) -> HoloHashed<DnaDef> {
        DnaDefHashed::from_content_sync(self.dna_def.clone())
    }

    async fn execute(&self) -> WorkflowResult<Outcome> {
        let dna_def = self.dna_def_hash();

        validate_op(
            self.op.as_ref().expect("No op set, invalid test case"),
            &dna_def,
            &self.cascade,
            None::<&MockDhtOpSender>,
        )
        .await
    }
}

fn test_op(previous: SignedHashed<Action>) -> DhtOp {
    let mut create_action = fixt!(Create);
    create_action.author = previous.action().author().clone();
    create_action.action_seq = previous.action().action_seq() + 1;
    create_action.prev_action = previous.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let action = Action::Create(create_action);

    DhtOp::RegisterAgentActivity(fixt!(Signature), action)
}
