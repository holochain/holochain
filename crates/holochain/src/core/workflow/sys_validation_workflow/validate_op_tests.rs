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
use crate::prelude::DnaHashFixturator;
use crate::prelude::Entry;
use crate::prelude::HoloHashed;
use crate::prelude::SignedActionHashed;
use crate::prelude::Timestamp;
use fixt::prelude::*;
use futures::FutureExt;
use hdk::prelude::Dna as HdkDna;
use holochain_cascade::CascadeSource;
use holochain_cascade::MockCascade;
use holochain_serialized_bytes::prelude::SerializedBytes;
use holochain_state::prelude::CreateFixturator;
use holochain_state::prelude::SignatureFixturator;
use holochain_types::EntryHashed;
use holochain_types::prelude::SignedActionHashedExt;
use holochain_zome_types::prelude::AgentValidationPkg;
use holochain_zome_types::prelude::AppEntry;
use holochain_zome_types::prelude::Create;
use holochain_zome_types::prelude::EntryVisibility;
use holochain_zome_types::record::Record;
use holochain_zome_types::record::SignedHashed;
use crate::prelude::EntryType;
use crate::prelude::CreateLinkFixturator;
use crate::prelude::ActionHashFixturator;
use holo_hash::HasHash;
use crate::prelude::AppEntryDef;
use crate::prelude::AppEntryBytesFixturator;
use holochain_state::prelude::AppEntryBytes;

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

    test_case.dna_def_mut().modifiers.origin_time =
        (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap();

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
        .times(1)
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
        .times(1)
        .returning(move |_hash, _options| {
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

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_from_network() {
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

    cascade.expect_retrieve_action().once().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
            let signed_action = signed_action.clone();
            async move {
                Ok(Some((
                    Record::new(signed_action, None),
                    CascadeSource::Network,
                )))
            }
            .boxed()
        });

    let mut sender = MockDhtOpSender::new();
    sender
        .expect_send_register_agent_activity()
        .times(1)
        .returning(move |_| async move { Ok(()) }.boxed());

    let validation_outcome = validate_op(&op, &dna_def, &cascade, Some(&sender))
        .await
        .unwrap();

    assert!(
        matches!(validation_outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        validation_outcome
    );
}

// TODO This should not error but also represents a missed opportunity to capture an op.
//      At the moment this is silently ignored because the `incoming_dht_ops_sender` is optional.
#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_from_network_but_missing_op_sender() {
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

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
            let signed_action = signed_action.clone();
            async move {
                Ok(Some((
                    Record::new(signed_action, None),
                    CascadeSource::Network,
                )))
            }
            .boxed()
        });

    let validation_outcome = validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>)
        .await
        .unwrap();

    assert!(
        matches!(validation_outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_action_not_found() {
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

    cascade.expect_retrieve_action().times(1).returning({
        move |_, _| {
            // Not found here, even though `retrieve` found it so not entirely realistic but good enough.
            async move { Ok(None) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::MissingDhtDep),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_author_mismatch_with_prev() {
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
    create_action.author = fixt!(AgentPubKey);
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let action = Action::Create(create_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_timestamp_same_as_prev() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    let common_timestamp = Timestamp::now();

    // This is the previous
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = agent.clone().into();
    validation_package_action.action_seq = 10;
    validation_package_action.timestamp = common_timestamp.clone().into();
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
    create_action.timestamp = common_timestamp.into();
    let action = Action::Create(create_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_timestamp_before_prev() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = agent.clone().into();
    validation_package_action.action_seq = 10;
    validation_package_action.timestamp = Timestamp::now().into();
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
    create_action.timestamp = (Timestamp::now() - std::time::Duration::from_secs(10)).unwrap().into();
    let action = Action::Create(create_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_seq_number_decrements() {
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
    create_action.action_seq = 9; // Should be 11, has gone down instead of up
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let action = Action::Create(create_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_seq_number_reused() {
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
    create_action.action_seq = 10; // Should be 11, but has been re-used
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let action = Action::Create(create_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_not_preceeded_by_avp() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = agent.clone().into();
    prev_create_action.action_seq = 10;
    prev_create_action.timestamp = Timestamp::now().into();
    let action = Action::Create(prev_create_action);
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
    create_action.entry_type = EntryType::AgentPubKey;
    let action = Action::Create(create_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
            let signed_action = signed_action.clone();
            async move {
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

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_avp_op_not_followed_by_create() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let action = AgentValidationPkg {
        author: agent.clone().into(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: fixt!(ActionHash),
        membrane_proof: None,
    };
    let action = Action::AgentValidationPkg(action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    // and current which needs values from previous
    let mut create_link_action = fixt!(CreateLink);
    create_link_action.author = signed_action.action().author().clone();
    create_link_action.action_seq = signed_action.action().action_seq() + 1;
    create_link_action.prev_action = signed_action.as_hash().clone();
    create_link_action.timestamp = Timestamp::now().into();
    let action = Action::CreateLink(create_link_action);

    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), action);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    cascade
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
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

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_store_entry_with_no_entry() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut action = fixt!(Create);
    action.author = agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let action = Action::Create(action);
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
    create_action.entry_type = EntryType::CapClaim;
    let action = Action::Create(create_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::NotStored);

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    let validation_outcome = validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>)
        .await
        .unwrap();

    assert!(
        matches!(validation_outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_entry_with_wrong_entry_type() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut action = fixt!(Create);
    action.author = agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let action = Action::Create(action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    let agent_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(agent_entry.clone());

    // and current which needs values from previous
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::AgentPubKey; // Claiming to be a public key but is actually an app entry
    create_action.entry_hash = entry_hash.as_hash().clone();
    let action = Action::Create(create_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(agent_entry));

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    let validation_outcome = validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>)
        .await
        .unwrap();

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_entry_with_wrong_entry_hash() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut action = fixt!(Create);
    action.author = agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let action = Action::Create(action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    let agent_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(agent_entry.clone());

    // Create some new data which will have a different hash
    let agent_entry = Entry::App(fixt!(AppEntryBytes));

    // and current which needs values from previous
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public));
    create_action.entry_hash = entry_hash.as_hash().clone();
    let action = Action::Create(create_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(agent_entry));

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    let validation_outcome = validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>)
        .await
        .unwrap();

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_large_entry() {
    holochain_trace::test_run().unwrap();

    use serde::{Serialize, Deserialize};
    use holochain_serialized_bytes::prelude::*;
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct TestLargeEntry {
        data: Vec<u8>,
    }

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut action = fixt!(Create);
    action.author = agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let action = Action::Create(action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    let agent_entry = Entry::App(AppEntryBytes(TestLargeEntry { data: vec![0; 100_000_000] }.try_into().unwrap()));
    let entry_hash = EntryHashed::from_content_sync(agent_entry.clone());

    // and current which needs values from previous
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public));
    create_action.entry_hash = entry_hash.as_hash().clone();
    let action = Action::Create(create_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(agent_entry));

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(1).returning({
        let signed_action = signed_action.clone();
        move |_, _| {
            let signed_action = signed_action.clone();
            async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
        }
    });

    let validation_outcome = validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>)
        .await
        .unwrap();

    assert!(
        matches!(validation_outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        validation_outcome
    );
}

// TODO add a test which validates an op that isn't a create or an update

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

    cascade.expect_retrieve_action().times(1).returning({
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
        .times(1)
        .returning(move |_hash, _options| {
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
