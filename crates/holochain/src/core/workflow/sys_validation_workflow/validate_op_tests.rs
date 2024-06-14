use std::collections::HashMap;
use std::sync::Arc;

use super::retrieve_previous_actions_for_ops;
use super::validation_deps::SysValDeps;
use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::core::workflow::sys_validation_workflow::validate_op;
use crate::core::workflow::WorkflowResult;
use crate::core::PrevActionErrorKind;
use crate::core::ValidationOutcome;
use crate::prelude::*;
use ::fixt::prelude::*;
use futures::FutureExt;
use hdk::prelude::Dna as HdkDna;
use holochain_cascade::CascadeSource;
use holochain_cascade::MockCascade;
use holochain_serialized_bytes::prelude::SerializedBytes;

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_dna_op() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    let dna_action = HdkDna {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        hash: test_case.dna_def_hash().hash,
    };
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action)).into();

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_dna_op_mismatched_dna_hash() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    let mut mismatched_dna_hash = fixt!(DnaHash);
    loop {
        if mismatched_dna_hash != test_case.dna_def_hash().hash {
            break;
        }
        mismatched_dna_hash = fixt!(DnaHash);
    }

    let dna_action = HdkDna {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        // Will not match the space hash from the test_case
        hash: mismatched_dna_hash.clone(),
    };
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action)).into();

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::WrongDna(
                mismatched_dna_hash,
                test_case.dna_def_hash().hash.clone(),
            )
            .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_dna_op_before_origin_time() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Put the origin time in the future so that ops created now shouldn't be valid.
    test_case.dna_def_mut().modifiers.origin_time =
        (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap();

    let dna_action = HdkDna {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        hash: test_case.dna_def_hash().hash,
    };
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action.clone())).into();

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (
                    PrevActionErrorKind::InvalidRootOriginTime,
                    Action::Dna(dna_action)
                )
                    .into()
            )
            .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn non_dna_op_as_first_action() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let dna_action = HdkDna {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        hash: test_case.dna_def_hash().hash,
    };
    let previous_action = test_case.sign_action(Action::Dna(dna_action)).await;

    let mut create = fixt!(Create);
    create.prev_action = previous_action.as_hash().clone();
    create.action_seq = 0; // Not valid, a DNA should always be first
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create.clone())).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (PrevActionErrorKind::InvalidRoot, Action::Create(create)).into()
            )
            .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_agent_validation_package_op() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let dna_action = HdkDna {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        hash: test_case.dna_def_hash().hash,
    };
    let previous_action = test_case.sign_action(Action::Dna(dna_action)).await;

    // Op to validate
    let action = AgentValidationPkg {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: previous_action.as_hash().clone(),
        membrane_proof: None,
    };
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::AgentValidationPkg(action)).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_delete_agent_key_op() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Create agent pub key action
    let create_agent_pub_key = Create {
        author: test_case.agent.clone(),
        action_seq: 2,
        entry_type: EntryType::AgentPubKey,
        entry_hash: test_case.agent.clone().into(),
        prev_action: fixt!(ActionHash),
        weight: Default::default(),
        timestamp: Timestamp::now().into(),
    };
    let create_agent_pub_key_action = test_case
        .sign_action(Action::Create(create_agent_pub_key))
        .await;

    // Op to validate
    let mut action = fixt!(Delete);
    action.author = test_case.agent.clone();
    action.prev_action = create_agent_pub_key_action.as_hash().clone();
    action.action_seq = create_agent_pub_key_action.action().action_seq() + 1;
    action.deletes_entry_address = test_case.agent.clone().into();
    action.timestamp = Timestamp::now();
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Delete(action)).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![create_agent_pub_key_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn reject_action_after_deleted_agent_key() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Delete agent pub key action
    let mut delete_agent_pub_key = fixt!(Delete);
    delete_agent_pub_key.author = test_case.agent.clone();
    delete_agent_pub_key.deletes_entry_address = test_case.agent.clone().into();
    delete_agent_pub_key.action_seq = 4;
    delete_agent_pub_key.timestamp = Timestamp::now();
    let delete_agent_pub_key_action = test_case
        .sign_action(Action::Delete(delete_agent_pub_key))
        .await;

    // Op to validate
    let op = test_op(&delete_agent_pub_key_action);

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![delete_agent_pub_key_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        outcome,
        Outcome::Rejected(ValidationOutcome::InvalidAgentKey(test_case.agent.clone()).to_string()),
        "Expected Rejected but actual outcome was {outcome:?}",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_create_op() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone();
    prev_create_action.action_seq = 10;
    prev_create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action)).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
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
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone();
    prev_create_action.action_seq = 10;
    prev_create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action)).into();

    test_case
        .cascade_mut()
        .expect_retrieve_action()
        .times(1)
        .returning(move |_, _| async move { Ok(None) }.boxed());

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert!(matches!(outcome, Outcome::MissingDhtDep));

    // Simulate the dep being found on the network
    test_case
        .current_validation_dependencies
        .same_dht
        .lock()
        .insert(previous_action, CascadeSource::Network);

    // Run again to process new ops from the network
    let outcome = test_case.run().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_action_not_found() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    let signed_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = signed_action.action().author().clone();
    create_action.action_seq = signed_action.action().action_seq() + 1;
    create_action.prev_action = signed_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action)).into();

    test_case
        .cascade_mut()
        .expect_retrieve_action()
        .times(1)
        .returning(move |_, _| async move { Ok(None) }.boxed());

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_author_mismatch_with_prev() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
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
    create_action.author = mismatched_author.clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action.clone()))
            .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (
                    PrevActionErrorKind::Author(
                        test_case.agent.clone(),
                        mismatched_author.clone(),
                    ),
                    Action::Create(create_action),
                )
                    .into(),
            )
            .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_timestamp_same_as_prev() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    let common_timestamp = Timestamp::now();

    // Previous action
    let mut init_action = fixt!(InitZomesComplete);
    init_action.author = test_case.agent.clone();
    init_action.action_seq = 10;
    init_action.timestamp = common_timestamp;
    let previous_action = test_case
        .sign_action(Action::InitZomesComplete(init_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = common_timestamp;
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action.clone()))
            .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome,);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_timestamp_before_prev() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    validation_package_action.timestamp = Timestamp::now();
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(
            validation_package_action.clone(),
        ))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = (Timestamp::now() - std::time::Duration::from_secs(10)).unwrap();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action.clone()))
            .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (
                    PrevActionErrorKind::Timestamp(
                        validation_package_action.timestamp,
                        create_action.timestamp,
                    ),
                    Action::Create(create_action),
                )
                    .into(),
            )
            .to_string()
        ),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_seq_number_decrements() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = 9; // Should be 11, has gone down instead of up
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action.clone()))
            .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (
                    PrevActionErrorKind::InvalidSeq(9, 10),
                    Action::Create(create_action),
                )
                    .into(),
            )
            .to_string()
        ),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_seq_number_reused() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(validation_package_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = 10; // Should be 11, but has been re-used
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action.clone()))
            .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (
                    PrevActionErrorKind::InvalidSeq(10, 10),
                    Action::Create(create_action),
                )
                    .into(),
            )
            .to_string()
        ),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_not_preceeded_by_avp() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone();
    prev_create_action.action_seq = 10;
    prev_create_action.timestamp = Timestamp::now();
    prev_create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action))
        .await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::AgentPubKey;
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action.clone()))
            .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::PrevActionError(
            (
                PrevActionErrorKind::InvalidSuccessor(
                    "Every Create or Update for an AgentPubKey must be preceded by an AgentValidationPkg".to_string(),
                    Box::new((
                        previous_action.action().clone(),
                        Action::Create(create_action.clone()),
                    )),
                ),
                Action::Create(create_action),
            )
                .into(),
        )
        .to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_avp_op_not_followed_by_create() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let action = AgentValidationPkg {
        author: test_case.agent.clone(),
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
    create_link_action.timestamp = Timestamp::now();
    let op = ChainOp::RegisterAgentActivity(
        fixt!(Signature),
        Action::CreateLink(create_link_action.clone()),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::PrevActionError(
            (
                PrevActionErrorKind::InvalidSuccessor(
                    "Every AgentValidationPkg must be followed by a Create or Update for an AgentPubKey".to_string(),
                    Box::new((
                        previous_action.action().clone(),
                        Action::CreateLink(create_link_action.clone()),
                    )),
                ),
                Action::CreateLink(create_link_action),
            )
                .into(),
        )
        .to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_store_record_with_no_entry() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::CapClaim;
    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        holochain_zome_types::record::RecordEntry::NotStored,
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_leaks_entry() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Private, // Private so should not have entry data
    });
    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        holochain_zome_types::record::RecordEntry::Present(Entry::App(fixt!(AppEntryBytes))), // but go ahead and provide the entry data anyway
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::PrivateEntryLeaked.to_string()),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_with_entry_having_wrong_entry_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let action = AgentValidationPkg {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: fixt!(ActionHash),
        membrane_proof: None,
    };
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(action))
        .await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::AgentPubKey; // Claiming to be a public key but is actually an app entry
    create_action.entry_hash = entry_hash.as_hash().clone();
    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::EntryTypeMismatch.to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_with_entry_having_wrong_entry_hash() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
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

    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        // Create some new data which will have a different hash
        holochain_zome_types::record::RecordEntry::Present(mismatched_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::EntryHash.to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_with_large_entry() {
    holochain_trace::test_run();

    use holochain_serialized_bytes::prelude::*;
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct TestLargeEntry {
        data: Vec<u8>,
    }

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
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
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    create_action.entry_hash = entry_hash.as_hash().clone();
    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::EntryTooLarge(5_000_011).to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_store_record_update() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone();
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
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = previous_action.action().author().clone();
    update_action.action_seq = previous_action.action().action_seq() + 1;
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
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
    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Update(update_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_update_prev_which_is_not_updateable() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Dna);
    to_update_action.author = test_case.agent.clone();
    let to_update_signed_action = test_case.sign_action(Action::Dna(to_update_action)).await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = previous_action.action().author().clone();
    update_action.action_seq = previous_action.action().action_seq() + 1;
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
    update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Update(update_action),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![
            to_update_signed_action.clone(),
            previous_action,
        ])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(to_update_signed_action.action().clone()).to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_update_changes_entry_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone();
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
        .sign_action(Action::Create(to_update_action.clone()))
        .await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = previous_action.action().author().clone();
    update_action.action_seq = previous_action.action().action_seq() + 1;
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
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
    let op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Update(update_action.clone()),
        holochain_zome_types::record::RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::UpdateTypeMismatch(
                to_update_action.entry_type,
                update_action.entry_type
            )
            .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_entry_having_wrong_entry_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::AgentPubKey; // Claiming to be a public key but is actually an app entry
    create_action.entry_hash = entry_hash.as_hash().clone();
    let op = ChainOp::StoreEntry(
        fixt!(Signature),
        holochain_types::action::NewEntryAction::Create(create_action),
        app_entry,
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::EntryTypeMismatch.to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_entry_having_wrong_entry_hash() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
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

    let op = ChainOp::StoreEntry(
        fixt!(Signature),
        holochain_types::action::NewEntryAction::Create(create_action),
        // Create some new data which will have a different hash
        mismatched_entry,
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::EntryHash.to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_with_large_entry() {
    holochain_trace::test_run();

    use holochain_serialized_bytes::prelude::*;
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct TestLargeEntry {
        data: Vec<u8>,
    }

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
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
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    create_action.entry_hash = entry_hash.as_hash().clone();
    let op = ChainOp::StoreEntry(
        fixt!(Signature),
        holochain_types::action::NewEntryAction::Create(create_action),
        app_entry,
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::EntryTooLarge(5_000_011).to_string()),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_store_entry_update() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone();
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
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = previous_action.action().author().clone();
    update_action.action_seq = previous_action.action().action_seq() + 1;
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
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
    let op = ChainOp::StoreEntry(
        fixt!(Signature),
        holochain_types::action::NewEntryAction::Update(update_action),
        app_entry,
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome,);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_update_prev_which_is_not_updateable() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Dna);
    to_update_action.author = test_case.agent.clone();
    let to_update_signed_action = test_case.sign_action(Action::Dna(to_update_action)).await;

    // Previous action
    let mut action = fixt!(Create);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = previous_action.action().author().clone();
    update_action.action_seq = previous_action.action().action_seq() + 1;
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
    update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let op = ChainOp::StoreEntry(
        fixt!(Signature),
        holochain_types::action::NewEntryAction::Update(update_action),
        app_entry,
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![
            to_update_signed_action.clone(),
            previous_action,
        ])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(to_update_signed_action.action().clone()).to_string()
        ),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_update_changes_entry_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone();
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
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    action.action_seq = 10;
    action.prev_action = fixt!(ActionHash);
    action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(Action::Create(action)).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.author = previous_action.action().author().clone();
    update_action.action_seq = previous_action.action().action_seq() + 1;
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
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
    let op = ChainOp::StoreEntry(
        fixt!(Signature),
        holochain_types::action::NewEntryAction::Update(update_action.clone()),
        app_entry,
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![
            to_update_signed_action.clone(),
            previous_action,
        ])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::UpdateTypeMismatch(
                to_update_signed_action
                    .action()
                    .entry_type()
                    .unwrap()
                    .clone(),
                update_action.entry_type
            )
            .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_register_updated_content() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone();
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

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
    update_action.entry_type = to_update_signed_action.hashed.entry_type().unwrap().clone();
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_entry_address = to_update_signed_action
        .action()
        .entry_hash()
        .unwrap()
        .clone();
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let op = ChainOp::RegisterUpdatedContent(
        fixt!(Signature),
        update_action,
        RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_updated_content_missing_updates_ref() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Needed to set up mocking but not actually referenced
    let mut dummy_prev_action = fixt!(Create);
    dummy_prev_action.author = test_case.agent.clone();
    dummy_prev_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_prev_action = test_case
        .sign_action(Action::Create(dummy_prev_action))
        .await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_prev_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let mut update_action: holochain_zome_types::prelude::Update = fixt!(Update);
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
    update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    update_action.original_action_address = mismatched_action_hash;
    let op = ChainOp::RegisterUpdatedContent(
        fixt!(Signature),
        update_action,
        RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_prev_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_register_updated_record() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Create);
    to_update_action.author = test_case.agent.clone();
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

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Update);
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
    update_action.entry_type = to_update_signed_action.hashed.entry_type().unwrap().clone();
    update_action.entry_hash = entry_hash.as_hash().clone();
    update_action.original_entry_address = to_update_signed_action
        .action()
        .entry_hash()
        .unwrap()
        .clone();
    update_action.original_action_address = to_update_signed_action.as_hash().clone();
    let op = ChainOp::RegisterUpdatedRecord(
        fixt!(Signature),
        update_action,
        RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_updated_record_missing_updates_ref() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Needed to set up mocking but not actually referenced
    let mut dummy_prev_action = fixt!(Create);
    dummy_prev_action.author = test_case.agent.clone();
    dummy_prev_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_prev_action = test_case
        .sign_action(Action::Create(dummy_prev_action))
        .await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_prev_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let mut update_action: holochain_zome_types::prelude::Update = fixt!(Update);
    update_action.prev_action = previous_action.as_hash().clone();
    update_action.timestamp = Timestamp::now();
    update_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    update_action.original_action_address = mismatched_action_hash;
    let op = ChainOp::RegisterUpdatedRecord(
        fixt!(Signature),
        update_action,
        RecordEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_prev_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_register_deleted_by() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Create);
    to_delete_action.author = test_case.agent.clone();
    to_delete_action.timestamp = Timestamp::now();
    to_delete_action.action_seq = 5;
    to_delete_action.prev_action = fixt!(ActionHash);
    to_delete_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    to_delete_action.entry_hash = fixt!(EntryHash);
    let to_delete_signed_action = test_case
        .sign_action(Action::Create(to_delete_action))
        .await;

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut delete_action = fixt!(Delete);
    delete_action.prev_action = previous_action.as_hash().clone();
    delete_action.timestamp = Timestamp::now();
    delete_action.deletes_address = to_delete_signed_action.as_hash().clone();
    let op = ChainOp::RegisterDeletedBy(fixt!(Signature), delete_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_delete_signed_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_deleted_by_with_missing_deletes_ref() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Dummy action to set up the mock, won't be referenced
    let mut dummy_action = fixt!(Create);
    dummy_action.author = test_case.agent.clone();
    dummy_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_action = test_case.sign_action(Action::Create(dummy_action)).await;

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Op to validate
    let mut delete_action = fixt!(Delete);
    delete_action.prev_action = previous_action.as_hash().clone();
    delete_action.timestamp = Timestamp::now();
    delete_action.deletes_address = mismatched_action_hash;
    let op = ChainOp::RegisterDeletedBy(fixt!(Signature), delete_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_deleted_by_wrong_delete_target() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Dna); // Cannot delete a DNA action
    to_delete_action.author = test_case.agent.clone();
    let to_delete_signed_action = test_case.sign_action(Action::Dna(to_delete_action)).await;

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut delete_action = fixt!(Delete);
    delete_action.prev_action = previous_action.action_address().clone();
    delete_action.timestamp = Timestamp::now();
    delete_action.deletes_address = to_delete_signed_action.as_hash().clone();
    let op = ChainOp::RegisterDeletedBy(fixt!(Signature), delete_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![
            to_delete_signed_action.clone(),
            previous_action,
        ])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(to_delete_signed_action.action().clone()).to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_register_deleted_entry_action() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Create);
    to_delete_action.author = test_case.agent.clone();
    to_delete_action.timestamp = Timestamp::now();
    to_delete_action.action_seq = 5;
    to_delete_action.prev_action = fixt!(ActionHash);
    to_delete_action.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    to_delete_action.entry_hash = fixt!(EntryHash);
    let to_delete_signed_action = test_case
        .sign_action(Action::Create(to_delete_action))
        .await;

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut delete_action = fixt!(Delete);
    delete_action.timestamp = Timestamp::now();
    delete_action.deletes_address = to_delete_signed_action.as_hash().clone();
    delete_action.prev_action = previous_action.as_hash().clone();
    let op = ChainOp::RegisterDeletedEntryAction(fixt!(Signature), delete_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_delete_signed_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_deleted_entry_action_with_missing_deletes_ref() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Dummy action to set up the mock, won't be referenced
    let mut dummy_action = fixt!(Create);
    dummy_action.author = test_case.agent.clone();
    dummy_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_action = test_case.sign_action(Action::Create(dummy_action)).await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut delete_action = fixt!(Delete);
    delete_action.prev_action = previous_action.as_hash().clone();
    delete_action.timestamp = Timestamp::now();
    delete_action.deletes_address = mismatched_action_hash;
    let op = ChainOp::RegisterDeletedEntryAction(fixt!(Signature), delete_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_deleted_entry_action_wrong_delete_target() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Dna); // Cannot delete a DNA action
    to_delete_action.author = test_case.agent.clone();
    let to_delete_signed_action = test_case.sign_action(Action::Dna(to_delete_action)).await;

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut delete_action = fixt!(Delete);
    delete_action.prev_action = previous_action.as_hash().clone();
    delete_action.timestamp = Timestamp::now();
    delete_action.deletes_address = to_delete_signed_action.as_hash().clone();
    let op = ChainOp::RegisterDeletedEntryAction(fixt!(Signature), delete_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![
            to_delete_signed_action.clone(),
            previous_action,
        ])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(to_delete_signed_action.action().clone()).to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_add_link() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut create_link_action = fixt!(CreateLink);
    create_link_action.prev_action = previous_action.as_hash().clone();
    create_link_action.tag = "hello".as_bytes().to_vec().into();
    create_link_action.timestamp = Timestamp::now();
    let op = ChainOp::RegisterAddLink(fixt!(Signature), create_link_action).into();

    // Note that no mocking is configured so the base and target addressed for the link aren't not going to be checked.
    // This is intentional as the validation isn't meant to check them but not very obvious from this test, hence the comment!
    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_add_link_tag_too_large() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut create_link_action = fixt!(CreateLink);
    create_link_action.tag = vec![0; 2_000].into();
    create_link_action.timestamp = Timestamp::now();
    let op = ChainOp::RegisterAddLink(fixt!(Signature), create_link_action).into();

    // Op to validate
    let mut create_link = fixt!(CreateLink);
    create_link.author = test_case.agent.clone();
    create_link.tag = vec![0; 2_000].into();
    create_link.prev_action = previous_action.action_address().clone();
    create_link.timestamp = Timestamp::now();
    let op = ChainOp::RegisterAddLink(fixt!(Signature), create_link).into();

    // Note that mocking is only configured to check if the previous action deleted the agent key.
    // Base and target addressed for the link are not going to be checked.
    // This is intentional as the validation isn't meant to check them but not very obvious from this test, hence the comment!
    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(ValidationOutcome::TagTooLarge(2_000).to_string()),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_remove_link() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(CreateLink);
    action.author = test_case.agent.clone();
    action.timestamp = Timestamp::now();
    let previous_action = test_case.sign_action(Action::CreateLink(action)).await;

    // Op to validate
    let mut delete_link_action = fixt!(DeleteLink);
    delete_link_action.prev_action = previous_action.as_hash().clone();
    delete_link_action.timestamp = Timestamp::now();
    delete_link_action.link_add_address = previous_action.as_hash().clone();
    let op = ChainOp::RegisterRemoveLink(fixt!(Signature), delete_link_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(Outcome::Accepted, outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_remove_link_missing_link_add_ref() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Dummy action to set up the mock, won't be referenced
    let mut dummy_action = fixt!(CreateLink);
    dummy_action.author = test_case.agent.clone();
    dummy_action.timestamp = Timestamp::now();
    let dummy_action = test_case
        .sign_action(Action::CreateLink(dummy_action))
        .await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Previous action needed for agent validity check
    let previous_action = SignedActionHashed::new_unchecked(fixt!(Action), fixt!(Signature));

    // Op to validate
    let mut delete_link_action = fixt!(DeleteLink);
    delete_link_action.prev_action = previous_action.as_hash().clone();
    delete_link_action.timestamp = Timestamp::now();
    delete_link_action.link_add_address = mismatched_action_hash;
    let op = ChainOp::RegisterRemoveLink(fixt!(Signature), delete_link_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_action, previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {:?}",
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_remove_link_with_wrong_target_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut previous_action = fixt!(Update);
    previous_action.author = test_case.agent.clone().into();
    previous_action.timestamp = Timestamp::now();
    let previous_action = test_case.sign_action(Action::Update(previous_action)).await;

    // Op to validate
    let mut delete_link_action = fixt!(DeleteLink);
    delete_link_action.prev_action = previous_action.as_hash().clone();
    delete_link_action.timestamp = Timestamp::now();
    delete_link_action.link_add_address = previous_action.as_hash().clone();
    let op = ChainOp::RegisterRemoveLink(fixt!(Signature), delete_link_action).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotCreateLink(previous_action.as_hash().clone()).to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn action_after_close_chain() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let dna_action = CloseChain {
        author: test_case.agent.clone(),
        timestamp: Timestamp::now(),
        action_seq: 23,
        prev_action: fixt!(ActionHash),
        new_dna_hash: fixt!(DnaHash),
    };
    let previous_action = test_case.sign_action(Action::CloseChain(dna_action)).await;

    let mut create = fixt!(Create);
    create.prev_action = previous_action.as_hash().clone();
    create.action_seq = 24;
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });

    // Use agent activity so that we'll validate the previous action
    let op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create.clone())).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (
                    PrevActionErrorKind::ActionAfterChainClose,
                    Action::Create(create)
                )
                    .into()
            )
            .to_string()
        ),
        outcome
    );
}

// TODO this hits code which claims to be unreachable. Clearly it isn't so investigate the code path.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "TODO fix this test"]
async fn crash_case() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let agent = keystore.new_sign_keypair_random().await.unwrap();

    // This is the previous
    let mut create_action = fixt!(AgentValidationPkg);
    create_action.author = agent.clone();
    create_action.timestamp = Timestamp::now();
    create_action.action_seq = 10;
    let action = Action::AgentValidationPkg(create_action);
    let action_hashed = ActionHashed::from_content_sync(action);
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();

    // and current which needs values from previous
    let op = test_op(&signed_action);

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
                // TODO this line creates the problem, expects a None value
                Ok(Some((
                    Record::new(signed_action, Some(Entry::Agent(fixt!(AgentPubKey)))),
                    CascadeSource::Local,
                )))
            }
            .boxed()
        });

    let validation_outcome = validate_op(&op, &dna_def, SysValDeps::default(), None)
        .await
        .unwrap();

    assert!(matches!(validation_outcome, Outcome::Accepted));
}

struct TestCase {
    op: Option<DhtOp>,
    keystore: holochain_keystore::MetaLairClient,
    cascade: MockCascade,
    current_validation_dependencies: SysValDeps,
    dna_def: DnaDef,
    agent: AgentPubKey,
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
            current_validation_dependencies: SysValDeps::default(),
            dna_def,
            agent,
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

    pub fn expect_retrieve_records_from_cascade(
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
                move |hash, _| match previous_actions.get(&hash).cloned() {
                    Some(action) => async move { Ok(Some((action, CascadeSource::Local))) }.boxed(),
                    None => async move { Ok(None) }.boxed(),
                }
            });

        self
    }

    async fn run(&mut self) -> WorkflowResult<Outcome> {
        let dna_def = self.dna_def_hash();

        // Swap out the cascade so we can move it into the workflow
        let mut new_cascade = MockCascade::new();
        std::mem::swap(&mut new_cascade, &mut self.cascade);

        let cascade = Arc::new(new_cascade);

        retrieve_previous_actions_for_ops(
            self.current_validation_dependencies.clone(),
            cascade.clone(),
            vec![self
                .op
                .as_ref()
                .expect("No op set, invalid test case")
                .clone()]
            .into_iter()
            .map(DhtOpHashed::from_content_sync),
        )
        .await;

        validate_op(
            self.op.as_ref().expect("No op set, invalid test case"),
            &dna_def,
            self.current_validation_dependencies.clone(),
            None,
        )
        .await
    }
}

fn test_op(previous: &SignedHashed<Action>) -> DhtOp {
    let mut create_action = fixt!(Create);
    create_action.author = previous.action().author().clone();
    create_action.action_seq = previous.action().action_seq() + 1;
    create_action.prev_action = previous.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let action = Action::Create(create_action);

    ChainOp::RegisterAgentActivity(fixt!(Signature), action).into()
}
