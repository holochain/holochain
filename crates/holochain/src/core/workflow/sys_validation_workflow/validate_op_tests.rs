use std::collections::HashMap;

use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::core::workflow::sys_validation_workflow::validate_op;
use crate::core::workflow::WorkflowResult;
use crate::core::MockDhtOpSender;
use crate::prelude::Action;
use crate::prelude::ActionHashFixturator;
use crate::prelude::ActionHashed;
use crate::prelude::AgentPubKeyFixturator;
use crate::prelude::AgentValidationPkgFixturator;
use crate::prelude::AppEntryBytesFixturator;
use crate::prelude::AppEntryDef;
use crate::prelude::CreateLinkFixturator;
use crate::prelude::DhtOp;
use crate::prelude::DnaDef;
use crate::prelude::DnaDefHashed;
use crate::prelude::DnaFixturator;
use crate::prelude::DnaHashFixturator;
use crate::prelude::Entry;
use crate::prelude::EntryHashFixturator;
use crate::prelude::EntryType;
use crate::prelude::HoloHashed;
use crate::prelude::SignedActionHashed;
use crate::prelude::Timestamp;
use crate::prelude::UpdateFixturator;
use fixt::prelude::*;
use futures::FutureExt;
use hdk::prelude::Dna as HdkDna;
use holo_hash::hash_type::Agent;
use holo_hash::HasHash;
use holo_hash::HoloHash;
use holochain_cascade::CascadeSource;
use holochain_cascade::MockCascade;
use holochain_serialized_bytes::prelude::SerializedBytes;
use holochain_state::prelude::AppEntryBytes;
use holochain_state::prelude::CreateFixturator;
use holochain_state::prelude::SignatureFixturator;
use holochain_types::EntryHashed;
use holochain_types::prelude::SignedActionHashedExt;
use holochain_types::EntryHashed;
use holochain_zome_types::prelude::AgentValidationPkg;
use holochain_zome_types::prelude::EntryVisibility;
use holochain_zome_types::record::Record;
use holochain_zome_types::record::SignedHashed;

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_dna_op() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    let dna_action = HdkDna {
        author: test_case.agent.clone().into(),
        timestamp: Timestamp::now().into(),
        hash: test_case.dna_def_hash().hash,
    };
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action));

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

    let mut test_case = TestCase::new().await;

    let mut mismatched_dna_hash = fixt!(DnaHash);
    loop {
        if mismatched_dna_hash != test_case.dna_def_hash().hash {
            break;
        }
        mismatched_dna_hash = fixt!(DnaHash);
    }

    let dna_action = HdkDna {
        author: test_case.agent.clone().into(),
        timestamp: Timestamp::now().into(),
        // Will not match the space hash from the test_case
        hash: mismatched_dna_hash,
    };
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action));

    let outcome = test_case.with_op(op).execute().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_dna_op_before_origin_time() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Put the origin time in the future so that ops created now shouldn't be valid.
    test_case.dna_def_mut().modifiers.origin_time =
        (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap();

    let dna_action = HdkDna {
        author: test_case.agent.clone().into(),
        timestamp: Timestamp::now().into(),
        hash: test_case.dna_def_hash().hash,
    };
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action));

    let outcome = test_case.with_op(op).execute().await.unwrap();

    // TODO this test assertion would be better if it was asserting the actual reason
    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn non_dna_op_as_first_action() {
    holochain_trace::test_run().unwrap();

    let mut create = fixt!(Create);
    create.action_seq = 0; // Not valid, a DNA should always be first
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create));

    let outcome = TestCase::new().await.with_op(op).execute().await.unwrap();

    assert!(matches!(outcome, Outcome::Rejected));
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_agent_validation_package_op() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let dna_action = HdkDna {
        author: test_case.agent.clone().into(),
        timestamp: Timestamp::now().into(),
        hash: test_case.dna_def_hash().hash,
    };
    let previous_action = test_case.sign_action(Action::Dna(dna_action)).await;

    // Op to validate
    let action = AgentValidationPkg {
        author: test_case.agent.clone().into(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: previous_action.as_hash().clone(),
        membrane_proof: None,
    };
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::AgentValidationPkg(action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_create_op() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_from_network() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    test_case
        .cascade_mut()
        .expect_retrieve_action()
        .times(1)
        .returning({
            let previous_action = previous_action.clone();
            move |_, _| {
                let previous_action = previous_action.clone();
                async move { Ok(Some((previous_action, CascadeSource::Local))) }.boxed()
            }
        });

    test_case
        .cascade_mut()
        .expect_retrieve()
        .times(1)
        .returning(move |_hash, _options| {
            let previous_action = previous_action.clone();
            async move {
                Ok(Some((
                    Record::new(previous_action, None),
                    CascadeSource::Network,
                )))
            }
            .boxed()
        });

    let outcome = test_case
        .with_incoming_ops_sender()
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

// TODO This should not error but also represents a missed opportunity to capture an op.
//      At the moment this is silently ignored because the `incoming_dht_ops_sender` is optional.
#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_from_network_but_missing_op_sender() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    let signed_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    test_case
        .cascade_mut()
        .expect_retrieve_action()
        .times(1)
        .returning({
            let signed_action = signed_action.clone();
            move |_, _| {
                let signed_action = signed_action.clone();
                async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
            }
        });

    test_case
        .cascade_mut()
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

    let outcome = test_case.with_op(op).execute().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_action_not_found() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    let signed_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    test_case
        .cascade_mut()
        .expect_retrieve_action()
        .times(1)
        .returning({
            move |_, _| {
                // Not found here, even though `retrieve` found it so not entirely realistic but good enough.
                async move { Ok(None) }.boxed()
            }
        });

    test_case
        .cascade_mut()
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

    let outcome = test_case.with_op(op).execute().await.unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_author_mismatch_with_prev() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    let mut mismatched_author = fixt!(AgentPubKey);
    loop {
        if mismatched_author != test_case.agent {
            break;
        }
        mismatched_author = fixt!(AgentPubKey);
    }

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = mismatched_author;
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_timestamp_same_as_prev() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    let common_timestamp = Timestamp::now();

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    validation_package_action.timestamp = common_timestamp.clone().into();
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = common_timestamp.into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_timestamp_before_prev() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    validation_package_action.timestamp = Timestamp::now().into();
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = (Timestamp::now() - std::time::Duration::from_secs(10))
        .unwrap()
        .into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_seq_number_decrements() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = 9; // Should be 11, has gone down instead of up
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_seq_number_reused() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone().into();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = 10; // Should be 11, but has been re-used
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_not_preceeded_by_avp() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone().into();
    prev_create_action.action_seq = 10;
    prev_create_action.timestamp = Timestamp::now().into();
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::AgentPubKey;
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_avp_op_not_followed_by_create() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let action = AgentValidationPkg {
        author: test_case.agent.clone().into(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: fixt!(ActionHash),
        membrane_proof: None,
    };
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(action))
        .await;

    // Op to validate
    let mut create_link_action = fixt!(CreateLink);
    create_link_action.author = previous_action.action().author().clone();
    create_link_action.action_seq = previous_action.action().action_seq() + 1;
    create_link_action.prev_action = previous_action.as_hash().clone();
    create_link_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::CreateLink(create_link_action));

    let outcome = test_case
        .expect_retrieve_and_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_store_entry_with_no_entry() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::CapClaim;
    let op = DhtOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        holochain_zome_types::record::RecordEntry::NotStored,
    );

    let outcome = test_case
        .expect_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_entry_with_wrong_entry_type() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::AgentPubKey; // Claiming to be a public key but is actually an app entry
    create_action.entry_hash = entry_hash.as_hash().clone();
    let op = DhtOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    );

    let outcome = test_case
        .expect_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_entry_with_wrong_entry_hash() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    create_action.entry_hash = entry_hash.as_hash().clone();

    let mut mismatched_entry = Entry::App(fixt!(AppEntryBytes));
    loop {
        if mismatched_entry != app_entry {
            break;
        }
        mismatched_entry = Entry::App(fixt!(AppEntryBytes));
    }

    let op = DhtOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        // Create some new data which will have a different hash
        holochain_zome_types::record::RecordEntry::Present(mismatched_entry),
    );

    let outcome = test_case
        .expect_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_large_entry() {
    holochain_trace::test_run().unwrap();

    use holochain_serialized_bytes::prelude::*;
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct TestLargeEntry {
        data: Vec<u8>,
    }

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(AppEntryBytes(
        TestLargeEntry {
            data: vec![0; 5_000_000],
        }
        .try_into()
        .unwrap(),
    ));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    create_action.entry_hash = entry_hash.as_hash().clone();
    let op = DhtOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    );

    let outcome = test_case
        .expect_retrieve_actions_from_cascade(vec![previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_store_entry_update() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone().into();
    to_update_action.timestamp = Timestamp::now();
    to_update_action.action_seq = 5;
    to_update_action.prev_action = fixt!(ActionHash);
    to_update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    to_update_action.entry_hash = fixt!(EntryHash);
    let to_update_signed_action = test_case
        .sign_action(Action::Create(to_update_action))
        .await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = previous_action.action().author().clone();
    update_action.action_seq = previous_action.action().action_seq() + 1;
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now().into();
    update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_entry_address = to_update_signed_action
        .action()
        .entry_hash()
        .unwrap()
        .clone();
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let op = DhtOp::StoreRecord(
        fixt!(Signature),
        Action::Update(update_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    );

    let outcome = test_case
        .expect_retrieve_actions_from_cascade(vec![to_update_signed_action, previous_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_update_prev_which_is_not_updateable() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Dna);
    to_update_action.author = test_case.agent.clone().into();
    let to_update_signed_action = test_case.sign_action(Action::Dna(to_update_action)).await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let signed_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = signed_action.action().author().clone();
    update_action.action_seq = signed_action.action().action_seq() + 1;
    update_action.prev_action = signed_action.as_hash().clone();
    update_action.timestamp = Timestamp::now().into();
    update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let op = DhtOp::StoreRecord(
        fixt!(Signature),
        Action::Update(update_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    );

    let outcome = test_case
        .expect_retrieve_actions_from_cascade(vec![to_update_signed_action, signed_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_update_changes_entry_type() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone().into();
    to_update_action.timestamp = Timestamp::now();
    to_update_action.action_seq = 5;
    to_update_action.prev_action = fixt!(ActionHash);
    to_update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    to_update_action.entry_hash = fixt!(EntryHash);
    let to_update_signed_action = test_case
        .sign_action(Action::Create(to_update_action))
        .await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    let signed_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = signed_action.action().author().clone();
    update_action.action_seq = signed_action.action().action_seq() + 1;
    update_action.prev_action = signed_action.as_hash().clone();
    update_action.timestamp = Timestamp::now().into();
    // Different entry type defined here
    update_action.entry_type = EntryType::App(AppEntryDef::new(
        10.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_entry_address = to_update_signed_action
        .action()
        .entry_hash()
        .unwrap()
        .clone();
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let op = DhtOp::StoreRecord(
        fixt!(Signature),
        Action::Update(update_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    );

    let outcome = test_case
        .expect_retrieve_actions_from_cascade(vec![to_update_signed_action, signed_action])
        .with_op(op)
        .execute()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Rejected),
        "Expected Rejected but actual outcome was {:?}",
        outcome
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

    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());

    // and current which needs values from previous
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::AgentPubKey; // Claiming to be a public key but is actually an app entry
    create_action.entry_hash = entry_hash.as_hash().clone();
    let action = Action::Create(create_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(app_entry));

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

    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());

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

    let app_entry = Entry::App(AppEntryBytes(TestLargeEntry { data: vec![0; 5_000_000] }.try_into().unwrap()));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());

    // and current which needs values from previous
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public));
    create_action.entry_hash = entry_hash.as_hash().clone();
    let action = Action::Create(create_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(app_entry));

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
async fn validate_valid_store_entry_update() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = agent.clone().into();
    to_update_action.timestamp = Timestamp::now();
    to_update_action.action_seq = 5;
    to_update_action.prev_action = fixt!(ActionHash);
    to_update_action.entry_type = EntryType::App(AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public));

    let to_update_action = Action::Create(to_update_action);
    let to_update_action_hashed = ActionHashed::from_content_sync(to_update_action);
    let to_update_signed_action = SignedActionHashed::sign(&keystore, to_update_action_hashed)
        .await
        .unwrap();

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

    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());

    // and current which needs values from previous
    let mut update_action = fixt!(Update);
    update_action.author = signed_action.action().author().clone();
    update_action.action_seq = signed_action.action().action_seq() + 1;
    update_action.prev_action = signed_action.as_hash().clone();
    update_action.timestamp = Timestamp::now().into();
    update_action.entry_type = EntryType::App(AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_entry_address = fixt!(EntryHash); // entry_hash.as_hash().clone(); // TODO not checked?
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let action = Action::Update(update_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(app_entry));

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(2).returning({
        let to_update_signed_action = to_update_signed_action.clone();
        let signed_action = signed_action.clone();
        move |hash, _| {
            if hash == to_update_signed_action.as_hash().clone() {
                let to_update_signed_action = to_update_signed_action.clone();
                async move { Ok(Some((to_update_signed_action, CascadeSource::Local))) }.boxed()
            } else if hash == signed_action.as_hash().clone() {
                let signed_action = signed_action.clone();
                async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
            } else {
                unreachable!()
            }
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
async fn validate_store_entry_update_prev_which_is_not_updateable() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is to be updated
    let mut to_update_action = fixt!(Dna);
    to_update_action.author = agent.clone().into();
    let to_update_action = Action::Dna(to_update_action);
    let to_update_action_hashed = ActionHashed::from_content_sync(to_update_action);
    let to_update_signed_action = SignedActionHashed::sign(&keystore, to_update_action_hashed)
        .await
        .unwrap();

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

    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());

    // and current which needs values from previous
    let mut update_action = fixt!(Update);
    update_action.author = signed_action.action().author().clone();
    update_action.action_seq = signed_action.action().action_seq() + 1;
    update_action.prev_action = signed_action.as_hash().clone();
    update_action.timestamp = Timestamp::now().into();
    update_action.entry_type = EntryType::App(AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_entry_address = fixt!(EntryHash); // entry_hash.as_hash().clone(); // TODO not checked?
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let action = Action::Update(update_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(app_entry));

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(2).returning({
        let to_update_signed_action = to_update_signed_action.clone();
        let signed_action = signed_action.clone();
        move |hash, _| {
            if hash == to_update_signed_action.as_hash().clone() {
                let to_update_signed_action = to_update_signed_action.clone();
                async move { Ok(Some((to_update_signed_action, CascadeSource::Local))) }.boxed()
            } else if hash == signed_action.as_hash().clone() {
                let signed_action = signed_action.clone();
                async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
            } else {
                unreachable!()
            }
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
async fn validate_store_entry_update_changes_entry_type() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = agent.clone().into();
    to_update_action.timestamp = Timestamp::now();
    to_update_action.action_seq = 5;
    to_update_action.prev_action = fixt!(ActionHash);
    to_update_action.entry_type = EntryType::App(AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public));

    let to_update_action = Action::Create(to_update_action);
    let to_update_action_hashed = ActionHashed::from_content_sync(to_update_action);
    let to_update_signed_action = SignedActionHashed::sign(&keystore, to_update_action_hashed)
        .await
        .unwrap();

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

    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());

    // and current which needs values from previous
    let mut update_action = fixt!(Update);
    update_action.author = signed_action.action().author().clone();
    update_action.action_seq = signed_action.action().action_seq() + 1;
    update_action.prev_action = signed_action.as_hash().clone();
    update_action.timestamp = Timestamp::now().into();
    // Different entry type defined here
    update_action.entry_type = EntryType::App(AppEntryDef::new(10.into(), 0.into(), EntryVisibility::Public));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_entry_address = fixt!(EntryHash); // entry_hash.as_hash().clone(); // TODO not checked?
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let action = Action::Update(update_action);

    let op = DhtOp::StoreRecord(fixt!(Signature), action, holochain_zome_types::record::RecordEntry::Present(app_entry));

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let mut cascade = MockCascade::new();

    cascade.expect_retrieve_action().times(2).returning({
        let to_update_signed_action = to_update_signed_action.clone();
        let signed_action = signed_action.clone();
        move |hash, _| {
            if hash == to_update_signed_action.as_hash().clone() {
                let to_update_signed_action = to_update_signed_action.clone();
                async move { Ok(Some((to_update_signed_action, CascadeSource::Local))) }.boxed()
            } else if hash == signed_action.as_hash().clone() {
                let signed_action = signed_action.clone();
                async move { Ok(Some((signed_action, CascadeSource::Local))) }.boxed()
            } else {
                unreachable!()
            }
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

struct TestCase {
    op: Option<DhtOp>,
    keystore: holochain_keystore::MetaLairClient,
    cascade: MockCascade,
    dna_def: DnaDef,
    agent: HoloHash<Agent>,
    incoming_ops_sender: Option<MockDhtOpSender>,
}

impl TestCase {
    async fn new() -> Self {
        let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);

        let keystore = holochain_keystore::test_keystore();
        let agent = keystore.new_sign_keypair_random().await.unwrap();

        TestCase {
            op: None,
            keystore,
            cascade: MockCascade::new(),
            dna_def,
            agent,
            incoming_ops_sender: None,
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

    pub async fn sign_action(&self, action: Action) -> SignedActionHashed {
        let action_hashed = ActionHashed::from_content_sync(action);
        SignedActionHashed::sign(&self.keystore, action_hashed)
            .await
            .unwrap()
    }

    pub fn expect_retrieve_actions_from_cascade(
        &mut self,
        previous_actions: Vec<SignedActionHashed>,
    ) -> &mut Self {
        let previous_actions = previous_actions
            .into_iter()
            .map(|a| (a.as_hash().clone(), a))
            .collect::<HashMap<_, _>>();
        self.cascade
            .expect_retrieve_action()
            .times(previous_actions.len())
            .returning({
                let previous_actions = previous_actions.clone();
                move |hash, _| {
                    let action = previous_actions.get(&hash).unwrap().clone();
                    async move { Ok(Some((action, CascadeSource::Local))) }.boxed()
                }
            });

        self
    }

    pub fn expect_retrieve_and_retrieve_actions_from_cascade(
        &mut self,
        previous_actions: Vec<SignedActionHashed>,
    ) -> &mut Self {
        self.expect_retrieve_actions_from_cascade(previous_actions.clone());

        let previous_actions = previous_actions
            .into_iter()
            .map(|a| (a.as_hash().clone(), a))
            .collect::<HashMap<_, _>>();
        self.cascade
            .expect_retrieve()
            .times(previous_actions.len())
            .returning(move |hash, _| {
                let action = previous_actions
                    .get(&hash.try_into().unwrap())
                    .unwrap()
                    .clone();
                async move { Ok(Some((Record::new(action, None), CascadeSource::Local))) }.boxed()
            });

        self
    }

    fn with_incoming_ops_sender(&mut self) -> &mut Self {
        let mut sender = MockDhtOpSender::new();
        sender
            .expect_send_register_agent_activity()
            .times(1)
            .returning(move |_| async move { Ok(()) }.boxed());

        self.incoming_ops_sender = Some(sender);

        self
    }

    async fn execute(&self) -> WorkflowResult<Outcome> {
        let dna_def = self.dna_def_hash();

        validate_op(
            self.op.as_ref().expect("No op set, invalid test case"),
            &dna_def,
            &self.cascade,
            self.incoming_ops_sender.as_ref(),
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
