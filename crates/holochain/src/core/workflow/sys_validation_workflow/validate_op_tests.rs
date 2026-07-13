use super::retrieve_previous_actions_for_ops;
use super::validation_deps::SysValDeps;
use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::core::workflow::sys_validation_workflow::validate_op;
use crate::core::workflow::WorkflowResult;
use crate::core::ValidationOutcome;
use crate::prelude::*;
use ::fixt::prelude::*;
use futures::FutureExt;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holochain_cascade::CascadeSource;
use holochain_cascade::MockCascade;
// This module builds op-pipeline types (`ChainOp`/`DhtOp`) directly and feeds
// them to `validate_op`. The `fixt!(Action, <Variant>Action)` fixturators yield
// v2 `Action` values (an `ActionHeader` + `ActionData` envelope) that are
// authored, hashed, and signed.
use holochain_types::dht_v2::{ChainOp, DhtOp, DhtOpHashed, OpEntry, SignedAction};
use holochain_zome_types::dht_v2::{Action, ActionData};
use holochain_zome_types::fixt::{
    ActionFixturator, AgentValidationPkgAction, CloseChainAction, CreateAction, CreateLinkAction,
    DeleteAction, DeleteLinkAction, DnaAction, InitZomesCompleteAction, UpdateAction,
};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_dna_op() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    let mut dna_action = fixt!(Action, DnaAction);
    dna_action.header.author = test_case.agent.clone();
    dna_action.header.timestamp = Timestamp::now();
    if let ActionData::Dna(d) = &mut dna_action.data {
        d.dna_hash = test_case.dna_def_hash().hash;
    }
    let op = ChainOp::AgentActivity(signed(dna_action, fixt!(Signature))).into();

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}"
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

    let mut dna_action = fixt!(Action, DnaAction);
    dna_action.header.author = test_case.agent.clone();
    dna_action.header.timestamp = Timestamp::now();
    // Will not match the space hash from the test_case
    if let ActionData::Dna(d) = &mut dna_action.data {
        d.dna_hash = mismatched_dna_hash.clone();
    }
    let op = ChainOp::AgentActivity(signed(dna_action, fixt!(Signature))).into();

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
async fn non_dna_op_as_first_action() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut dna_action = fixt!(Action, DnaAction);
    dna_action.header.author = test_case.agent.clone();
    dna_action.header.timestamp = Timestamp::now();
    if let ActionData::Dna(d) = &mut dna_action.data {
        d.dna_hash = test_case.dna_def_hash().hash;
    }
    let previous_action = test_case.sign_action(dna_action).await;

    let mut create = fixt!(Action, CreateAction);
    create.header.prev_action = Some(previous_action.as_hash().clone());
    create.header.action_seq = 0; // Not valid, a DNA should always be first
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create.clone(), fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError((PrevActionErrorKind::InvalidRoot, create,).into())
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
    let mut dna_action = fixt!(Action, DnaAction);
    dna_action.header.author = test_case.agent.clone();
    dna_action.header.timestamp = Timestamp::now();
    if let ActionData::Dna(d) = &mut dna_action.data {
        d.dna_hash = test_case.dna_def_hash().hash;
    }
    let previous_action = test_case.sign_action(dna_action).await;

    // Op to validate
    let mut action = fixt!(Action, AgentValidationPkgAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 1;
    action.header.prev_action = Some(previous_action.as_hash().clone());
    if let ActionData::AgentValidationPkg(d) = &mut action.data {
        d.membrane_proof = None;
    }
    let op = ChainOp::AgentActivity(signed(action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_create_op() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut prev_create_action = fixt!(Action, CreateAction);
    prev_create_action.header.author = test_case.agent.clone();
    prev_create_action.header.action_seq = 10;
    *prev_create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(prev_create_action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_from_network() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut prev_create_action = fixt!(Action, CreateAction);
    prev_create_action.header.author = test_case.agent.clone();
    prev_create_action.header.action_seq = 10;
    *prev_create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(prev_create_action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action, fixt!(Signature))).into();

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
        .lock()
        .expect("poisoned")
        .insert_action(previous_action, CascadeSource::Network);

    // Run again to process new ops from the network
    let outcome = test_case.run().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_with_prev_action_not_found() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(Action, AgentValidationPkgAction);
    validation_package_action.header.author = test_case.agent.clone();
    validation_package_action.header.action_seq = 10;
    let signed_action = test_case.sign_action(validation_package_action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = signed_action.action().author().clone();
    create_action.header.action_seq = signed_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(signed_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action, fixt!(Signature))).into();

    test_case
        .cascade_mut()
        .expect_retrieve_action()
        .times(1)
        .returning(move |_, _| async move { Ok(None) }.boxed());

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_create_op_author_mismatch_with_prev() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut validation_package_action = fixt!(Action, AgentValidationPkgAction);
    validation_package_action.header.author = test_case.agent.clone();
    validation_package_action.header.action_seq = 10;
    let previous_action = test_case.sign_action(validation_package_action).await;

    let mut mismatched_author = fixt!(AgentPubKey);
    loop {
        if mismatched_author != test_case.agent {
            break;
        }
        mismatched_author = fixt!(AgentPubKey);
    }

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = mismatched_author.clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action.clone(), fixt!(Signature))).into();

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
                    create_action,
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
    let mut init_action = fixt!(Action, InitZomesCompleteAction);
    init_action.header.author = test_case.agent.clone();
    init_action.header.action_seq = 10;
    init_action.header.timestamp = common_timestamp;
    let previous_action = test_case.sign_action(init_action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = common_timestamp;
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action, fixt!(Signature))).into();

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
    let mut validation_package_action = fixt!(Action, AgentValidationPkgAction);
    validation_package_action.header.author = test_case.agent.clone();
    validation_package_action.header.action_seq = 10;
    validation_package_action.header.timestamp = Timestamp::now();
    let previous_action = test_case
        .sign_action(validation_package_action.clone())
        .await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp =
        (Timestamp::now() - std::time::Duration::from_secs(10)).unwrap();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action.clone(), fixt!(Signature))).into();

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
                        validation_package_action.timestamp(),
                        create_action.timestamp(),
                    ),
                    create_action,
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
    let mut validation_package_action = fixt!(Action, AgentValidationPkgAction);
    validation_package_action.header.author = test_case.agent.clone();
    validation_package_action.header.action_seq = 10;
    let previous_action = test_case.sign_action(validation_package_action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = 9; // Should be 11, has gone down instead of up
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action.clone(), fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (PrevActionErrorKind::InvalidSeq(9, 10), create_action,).into(),
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
    let mut validation_package_action = fixt!(Action, AgentValidationPkgAction);
    validation_package_action.header.author = test_case.agent.clone();
    validation_package_action.header.action_seq = 10;
    let previous_action = test_case.sign_action(validation_package_action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = 10; // Should be 11, but has been re-used
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(signed(create_action.clone(), fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (PrevActionErrorKind::InvalidSeq(10, 10), create_action,).into(),
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
    let mut prev_create_action = fixt!(Action, CreateAction);
    prev_create_action.header.author = test_case.agent.clone();
    prev_create_action.header.action_seq = 10;
    prev_create_action.header.timestamp = Timestamp::now();
    *prev_create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(prev_create_action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::AgentPubKey;
    let op = ChainOp::AgentActivity(signed(create_action.clone(), fixt!(Signature))).into();

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
                        create_action.clone(),
                    )),
                ),
                create_action,
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
    let mut action = fixt!(Action, AgentValidationPkgAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 1;
    action.header.prev_action = Some(fixt!(ActionHash));
    if let ActionData::AgentValidationPkg(d) = &mut action.data {
        d.membrane_proof = None;
    }
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let mut create_link_action = fixt!(Action, CreateLinkAction);
    create_link_action.header.author = previous_action.action().author().clone();
    create_link_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_link_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_link_action.header.timestamp = Timestamp::now();
    let op = ChainOp::AgentActivity(signed(create_link_action.clone(), fixt!(Signature))).into();

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
                        create_link_action.clone(),
                    )),
                ),
                create_link_action,
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
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::CapClaim;
    let op = ChainOp::CreateRecord(
        signed(create_action, fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::NotStored),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_leaks_entry() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Private, // Private so should not have entry data
    });
    let op = ChainOp::CreateRecord(
        signed(create_action, fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::Present(
            Entry::App(fixt!(AppEntryBytes)),
        )),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

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
    let mut action = fixt!(Action, AgentValidationPkgAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 1;
    action.header.prev_action = Some(fixt!(ActionHash));
    if let ActionData::AgentValidationPkg(d) = &mut action.data {
        d.membrane_proof = None;
    }
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::AgentPubKey; // Claiming to be a public key but is actually an app entry
    *create_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    let op = ChainOp::CreateRecord(
        signed(create_action, fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::Present(
            app_entry,
        )),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

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
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *create_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();

    let mut mismatched_entry = Entry::App(fixt!(AppEntryBytes));
    loop {
        if mismatched_entry != app_entry {
            break;
        }
        mismatched_entry = Entry::App(fixt!(AppEntryBytes));
    }

    let op = ChainOp::CreateRecord(
        signed(create_action, fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::Present(
            mismatched_entry,
        )),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

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
    // A serializable entry whose `data` is sized past `MAX_ENTRY_SIZE` to
    // exercise the entry-size limit.
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct TestLargeEntry {
        data: Vec<u8>,
    }

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(AppEntryBytes(
        TestLargeEntry {
            data: vec![0; 5_000_000],
        }
        .try_into()
        .unwrap(),
    ));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *create_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    let op = ChainOp::CreateRecord(
        signed(create_action, fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::Present(
            app_entry,
        )),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

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
    let mut to_update_action = fixt!(Action, CreateAction);
    to_update_action.header.author = test_case.agent.clone();
    to_update_action.header.timestamp = Timestamp::now();
    to_update_action.header.action_seq = 5;
    to_update_action.header.prev_action = Some(fixt!(ActionHash));
    *to_update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_update_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_update_signed_action = test_case.sign_action(to_update_action).await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.author = previous_action.action().author().clone();
    update_action.header.action_seq = previous_action.action().action_seq() + 1;
    update_action.header.prev_action = Some(previous_action.as_hash().clone());
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_entry_address = to_update_signed_action
            .hashed
            .content
            .entry_hash()
            .unwrap()
            .clone();
    }
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::CreateRecord(
        signed(update_action, fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::Present(
            app_entry,
        )),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_update_prev_which_is_not_updateable() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Action, DnaAction);
    to_update_action.header.author = test_case.agent.clone();
    let to_update_signed_action = test_case.sign_action(to_update_action).await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.author = previous_action.action().author().clone();
    update_action.header.action_seq = previous_action.action().action_seq() + 1;
    update_action.header.prev_action = Some(previous_action.as_hash().clone());
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::CreateRecord(
        signed(update_action, fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::Present(
            app_entry,
        )),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(Box::new(to_update_signed_action.action().clone()))
                .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_record_update_changes_entry_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Action, CreateAction);
    to_update_action.header.author = test_case.agent.clone();
    to_update_action.header.timestamp = Timestamp::now();
    to_update_action.header.action_seq = 5;
    to_update_action.header.prev_action = Some(fixt!(ActionHash));
    *to_update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_update_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_update_signed_action = test_case.sign_action(to_update_action.clone()).await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.author = previous_action.action().author().clone();
    update_action.header.action_seq = previous_action.action().action_seq() + 1;
    update_action.header.prev_action = Some(previous_action.as_hash().clone());
    update_action.header.timestamp = Timestamp::now();
    // Different entry type defined here
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        10.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_entry_address = to_update_signed_action
            .hashed
            .content
            .entry_hash()
            .unwrap()
            .clone();
    }
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::CreateRecord(
        signed(update_action.clone(), fixt!(Signature)),
        to_op_entry(holochain_zome_types::record::RecordEntry::Present(
            app_entry,
        )),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::UpdateTypeMismatch(
                to_update_action.entry_type().unwrap().clone(),
                update_action.entry_type().unwrap().clone()
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
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::AgentPubKey; // Claiming to be a public key but is actually an app entry
    *create_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    let op = ChainOp::CreateEntry(
        signed(create_action, fixt!(Signature)),
        OpEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

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
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *create_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();

    let mut mismatched_entry = Entry::App(fixt!(AppEntryBytes));
    loop {
        if mismatched_entry != app_entry {
            break;
        }
        mismatched_entry = Entry::App(fixt!(AppEntryBytes));
    }

    let op = ChainOp::CreateEntry(
        signed(create_action, fixt!(Signature)),
        OpEntry::Present(mismatched_entry),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

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
    // A serializable entry whose `data` is sized past `MAX_ENTRY_SIZE` to
    // exercise the entry-size limit.
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    struct TestLargeEntry {
        data: Vec<u8>,
    }

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(AppEntryBytes(
        TestLargeEntry {
            data: vec![0; 5_000_000],
        }
        .try_into()
        .unwrap(),
    ));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *create_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    let op = ChainOp::CreateEntry(
        signed(create_action, fixt!(Signature)),
        OpEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case.with_op(op).run().await.unwrap();

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
    let mut to_update_action = fixt!(Action, CreateAction);
    to_update_action.header.author = test_case.agent.clone();
    to_update_action.header.timestamp = Timestamp::now();
    to_update_action.header.action_seq = 5;
    to_update_action.header.prev_action = Some(fixt!(ActionHash));
    *to_update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_update_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_update_signed_action = test_case.sign_action(to_update_action).await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.author = previous_action.action().author().clone();
    update_action.header.action_seq = previous_action.action().action_seq() + 1;
    update_action.header.prev_action = Some(previous_action.as_hash().clone());
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_entry_address = to_update_signed_action
            .hashed
            .content
            .entry_hash()
            .unwrap()
            .clone();
    }
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::CreateEntry(
        signed(update_action, fixt!(Signature)),
        OpEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action])
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
    let mut to_update_action = fixt!(Action, DnaAction);
    to_update_action.header.author = test_case.agent.clone();
    let to_update_signed_action = test_case.sign_action(to_update_action).await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.author = previous_action.action().author().clone();
    update_action.header.action_seq = previous_action.action().action_seq() + 1;
    update_action.header.prev_action = Some(previous_action.as_hash().clone());
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::CreateEntry(
        signed(update_action, fixt!(Signature)),
        OpEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(Box::new(to_update_signed_action.action().clone()))
                .to_string()
        ),
        outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_store_entry_update_changes_entry_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Action, CreateAction);
    to_update_action.header.author = test_case.agent.clone();
    to_update_action.header.timestamp = Timestamp::now();
    to_update_action.header.action_seq = 5;
    to_update_action.header.prev_action = Some(fixt!(ActionHash));
    *to_update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_update_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_update_signed_action = test_case.sign_action(to_update_action).await;

    // Previous action
    let mut action = fixt!(Action, CreateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 10;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.author = previous_action.action().author().clone();
    update_action.header.action_seq = previous_action.action().action_seq() + 1;
    update_action.header.prev_action = Some(previous_action.as_hash().clone());
    update_action.header.timestamp = Timestamp::now();
    // Different entry type defined here
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        10.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_entry_address = to_update_signed_action
            .hashed
            .content
            .entry_hash()
            .unwrap()
            .clone();
    }
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::CreateEntry(
        signed(update_action.clone(), fixt!(Signature)),
        OpEntry::Present(app_entry),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::UpdateTypeMismatch(
                to_update_signed_action
                    .hashed
                    .content
                    .entry_type()
                    .unwrap()
                    .clone(),
                update_action.entry_type().unwrap().clone()
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
    let mut to_update_action = fixt!(Action, CreateAction);
    to_update_action.header.author = test_case.agent.clone();
    to_update_action.header.timestamp = Timestamp::now();
    to_update_action.header.action_seq = 5;
    to_update_action.header.prev_action = Some(fixt!(ActionHash));
    *to_update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_update_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_update_signed_action = test_case.sign_action(to_update_action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() =
        to_update_signed_action.hashed.entry_type().unwrap().clone();
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_entry_address = to_update_signed_action
            .hashed
            .content
            .entry_hash()
            .unwrap()
            .clone();
    }
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::UpdateEntry(
        signed(update_action, fixt!(Signature)),
        to_op_entry(RecordEntry::Present(app_entry)),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action])
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
    let mut dummy_prev_action = fixt!(Action, CreateAction);
    dummy_prev_action.header.author = test_case.agent.clone();
    *dummy_prev_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_prev_action = test_case.sign_action(dummy_prev_action).await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_prev_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = mismatched_action_hash;
    }
    let op = ChainOp::UpdateEntry(
        signed(update_action, fixt!(Signature)),
        to_op_entry(RecordEntry::Present(app_entry)),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_prev_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_register_updated_record() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_update_action = fixt!(Action, CreateAction);
    to_update_action.header.author = test_case.agent.clone();
    to_update_action.header.timestamp = Timestamp::now();
    to_update_action.header.action_seq = 5;
    to_update_action.header.prev_action = Some(fixt!(ActionHash));
    *to_update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_update_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_update_signed_action = test_case.sign_action(to_update_action).await;

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let entry_hash = EntryHashed::from_content_sync(app_entry.clone());
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() =
        to_update_signed_action.hashed.entry_type().unwrap().clone();
    *update_action.entry_hash_mut().unwrap() = entry_hash.as_hash().clone();
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_entry_address = to_update_signed_action
            .hashed
            .content
            .entry_hash()
            .unwrap()
            .clone();
    }
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = to_update_signed_action.as_hash().clone();
    }
    let op = ChainOp::UpdateRecord(
        signed(update_action, fixt!(Signature)),
        to_op_entry(RecordEntry::Present(app_entry)),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_update_signed_action])
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
    let mut dummy_prev_action = fixt!(Action, CreateAction);
    dummy_prev_action.header.author = test_case.agent.clone();
    *dummy_prev_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_prev_action = test_case.sign_action(dummy_prev_action).await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_prev_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Op to validate
    let app_entry = Entry::App(fixt!(AppEntryBytes));
    let mut update_action = fixt!(Action, UpdateAction);
    update_action.header.timestamp = Timestamp::now();
    *update_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    if let ActionData::Update(d) = &mut update_action.data {
        d.original_action_address = mismatched_action_hash;
    }
    let op = ChainOp::UpdateRecord(
        signed(update_action, fixt!(Signature)),
        to_op_entry(RecordEntry::Present(app_entry)),
    )
    .into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_prev_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_register_deleted_by() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Action, CreateAction);
    to_delete_action.header.author = test_case.agent.clone();
    to_delete_action.header.timestamp = Timestamp::now();
    to_delete_action.header.action_seq = 5;
    to_delete_action.header.prev_action = Some(fixt!(ActionHash));
    *to_delete_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_delete_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_delete_signed_action = test_case.sign_action(to_delete_action).await;

    // Op to validate
    let mut delete_action = fixt!(Action, DeleteAction);
    delete_action.header.timestamp = Timestamp::now();
    if let ActionData::Delete(d) = &mut delete_action.data {
        d.deletes_address = to_delete_signed_action.as_hash().clone();
    }
    let op = ChainOp::DeleteRecord(signed(delete_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_delete_signed_action])
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
    let mut dummy_action = fixt!(Action, CreateAction);
    dummy_action.header.author = test_case.agent.clone();
    *dummy_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_action = test_case.sign_action(dummy_action).await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Op to validate
    let mut delete_action = fixt!(Action, DeleteAction);
    delete_action.header.timestamp = Timestamp::now();
    if let ActionData::Delete(d) = &mut delete_action.data {
        d.deletes_address = mismatched_action_hash;
    }
    let op = ChainOp::DeleteRecord(signed(delete_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_deleted_by_wrong_delete_target() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Action, DnaAction); // Cannot delete a DNA action
    to_delete_action.header.author = test_case.agent.clone();
    let to_delete_signed_action = test_case.sign_action(to_delete_action).await;

    // Op to validate
    let mut delete_action = fixt!(Action, DeleteAction);
    delete_action.header.timestamp = Timestamp::now();
    if let ActionData::Delete(d) = &mut delete_action.data {
        d.deletes_address = to_delete_signed_action.as_hash().clone();
    }
    let op = ChainOp::DeleteRecord(signed(delete_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_delete_signed_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(Box::new(to_delete_signed_action.action().clone()))
                .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_register_deleted_entry_action() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Action, CreateAction);
    to_delete_action.header.author = test_case.agent.clone();
    to_delete_action.header.timestamp = Timestamp::now();
    to_delete_action.header.action_seq = 5;
    to_delete_action.header.prev_action = Some(fixt!(ActionHash));
    *to_delete_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *to_delete_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let to_delete_signed_action = test_case.sign_action(to_delete_action).await;

    // Op to validate
    let mut delete_action = fixt!(Action, DeleteAction);
    delete_action.header.timestamp = Timestamp::now();
    if let ActionData::Delete(d) = &mut delete_action.data {
        d.deletes_address = to_delete_signed_action.as_hash().clone();
    }
    let op = ChainOp::DeleteEntry(signed(delete_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_delete_signed_action])
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
    let mut dummy_action = fixt!(Action, CreateAction);
    dummy_action.header.author = test_case.agent.clone();
    *dummy_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let dummy_action = test_case.sign_action(dummy_action).await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Op to validate
    let mut delete_action = fixt!(Action, DeleteAction);
    delete_action.header.timestamp = Timestamp::now();
    if let ActionData::Delete(d) = &mut delete_action.data {
        d.deletes_address = mismatched_action_hash;
    }
    let op = ChainOp::DeleteEntry(signed(delete_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_register_deleted_entry_action_wrong_delete_target() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Action to be updated
    let mut to_delete_action = fixt!(Action, DnaAction); // Cannot delete a DNA action
    to_delete_action.header.author = test_case.agent.clone();
    let to_delete_signed_action = test_case.sign_action(to_delete_action).await;

    // Op to validate
    let mut delete_action = fixt!(Action, DeleteAction);
    delete_action.header.timestamp = Timestamp::now();
    if let ActionData::Delete(d) = &mut delete_action.data {
        d.deletes_address = to_delete_signed_action.as_hash().clone();
    }
    let op = ChainOp::DeleteEntry(signed(delete_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![to_delete_signed_action.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(Box::new(to_delete_signed_action.action().clone()))
                .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_delete_a_delete_is_rejected() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Delete action to be deleted.
    let mut delete = fixt!(Action, DeleteAction);
    delete.header.author = test_case.agent.clone();
    let delete_action_signed_hashed = test_case.sign_action(delete).await;

    // Op to validate
    let mut delete_delete_action = fixt!(Action, DeleteAction);
    delete_delete_action.header.author = test_case.agent.clone();
    delete_delete_action.header.timestamp = Timestamp::now();
    if let ActionData::Delete(d) = &mut delete_delete_action.data {
        d.deletes_address = delete_action_signed_hashed.as_hash().clone();
    }

    // Validate a deleted entry action.
    let op = ChainOp::DeleteEntry(signed(delete_delete_action.clone(), fixt!(Signature))).into();
    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![delete_action_signed_hashed.clone()])
        .with_op(op)
        .run()
        .await
        .unwrap();
    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(Box::new(delete_action_signed_hashed.action().clone()))
                .to_string()
        ),
        outcome
    );

    // Validate a deleted by.
    let op = ChainOp::DeleteRecord(signed(delete_delete_action, fixt!(Signature))).into();
    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![])
        .with_op(op)
        .run()
        .await
        .unwrap();
    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::NotNewEntry(Box::new(delete_action_signed_hashed.action().clone()))
                .to_string()
        ),
        outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_add_link() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Op to validate
    let mut create_link_action = fixt!(Action, CreateLinkAction);
    if let ActionData::CreateLink(d) = &mut create_link_action.data {
        d.tag = "hello".as_bytes().to_vec().into();
    }
    create_link_action.header.timestamp = Timestamp::now();
    let op = ChainOp::CreateLink(signed(create_link_action, fixt!(Signature))).into();

    // Note that no mocking is configured so the base and target addressed for the link aren't not going to be checked.
    // This is intentional as the validation isn't meant to check them but not very obvious from this test, hence the comment!
    let outcome = test_case.with_op(op).run().await.unwrap();

    assert_eq!(Outcome::Accepted, outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_add_link_tag_too_large() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Op to validate
    let mut create_link_action = fixt!(Action, CreateLinkAction);
    if let ActionData::CreateLink(d) = &mut create_link_action.data {
        d.tag = vec![0; 2_000].into();
    }
    create_link_action.header.timestamp = Timestamp::now();
    let op = ChainOp::CreateLink(signed(create_link_action, fixt!(Signature))).into();

    // Note that no mocking is configured so the base and target addressed for the link aren't not going to be checked.
    // This is intentional as the validation isn't meant to check them but not very obvious from this test, hence the comment!
    let outcome = test_case.with_op(op).run().await.unwrap();

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
    let mut action = fixt!(Action, CreateLinkAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let mut delete_link_action = fixt!(Action, DeleteLinkAction);
    delete_link_action.header.timestamp = Timestamp::now();
    if let ActionData::DeleteLink(d) = &mut delete_link_action.data {
        d.link_add_address = previous_action.as_hash().clone();
    }
    let op = ChainOp::DeleteLink(signed(delete_link_action, fixt!(Signature))).into();

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
    let mut dummy_action = fixt!(Action, CreateLinkAction);
    dummy_action.header.author = test_case.agent.clone();
    dummy_action.header.timestamp = Timestamp::now();
    let dummy_action = test_case.sign_action(dummy_action).await;

    let mut mismatched_action_hash = fixt!(ActionHash);
    loop {
        if dummy_action.as_hash() != &mismatched_action_hash {
            break;
        }
        mismatched_action_hash = fixt!(ActionHash);
    }

    // Op to validate
    let mut delete_link_action = fixt!(Action, DeleteLinkAction);
    delete_link_action.header.timestamp = Timestamp::now();
    if let ActionData::DeleteLink(d) = &mut delete_link_action.data {
        d.link_add_address = mismatched_action_hash;
    }
    let op = ChainOp::DeleteLink(signed(delete_link_action, fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![dummy_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {outcome:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_remove_link_with_wrong_target_type() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous action
    let mut action = fixt!(Action, UpdateAction);
    action.header.author = test_case.agent.clone();
    action.header.timestamp = Timestamp::now();
    let previous_action = test_case.sign_action(action).await;

    // Op to validate
    let mut delete_link_action = fixt!(Action, DeleteLinkAction);
    delete_link_action.header.timestamp = Timestamp::now();
    if let ActionData::DeleteLink(d) = &mut delete_link_action.data {
        d.link_add_address = previous_action.as_hash().clone();
    }
    let op = ChainOp::DeleteLink(signed(delete_link_action, fixt!(Signature))).into();

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
    let mut dna_action = fixt!(Action, CloseChainAction);
    dna_action.header.author = test_case.agent.clone();
    dna_action.header.timestamp = Timestamp::now();
    dna_action.header.action_seq = 23;
    dna_action.header.prev_action = Some(fixt!(ActionHash));
    if let ActionData::CloseChain(d) = &mut dna_action.data {
        d.new_target = Some(fixt!(MigrationTarget));
    }

    // If this is an agent migration, the agent keypair needs to exist
    // so the Close can be signed.
    if let ActionData::CloseChain(d) = &mut dna_action.data {
        if let Some(MigrationTarget::Agent(agent)) = d.new_target.as_mut() {
            *agent = test_case.keystore.new_sign_keypair_random().await.unwrap();
        }
    }

    let previous_action = test_case.sign_action(dna_action).await;

    let mut create = fixt!(Action, CreateAction);
    create.header.prev_action = Some(previous_action.as_hash().clone());
    create.header.action_seq = 24;
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });

    // Use agent activity so that we'll validate the previous action
    let op = ChainOp::AgentActivity(signed(create.clone(), fixt!(Signature))).into();

    let outcome = test_case
        .expect_retrieve_records_from_cascade(vec![previous_action])
        .with_op(op)
        .run()
        .await
        .unwrap();

    assert_eq!(
        Outcome::Rejected(
            ValidationOutcome::PrevActionError(
                (PrevActionErrorKind::ActionAfterChainClose, create).into()
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
    let mut create_action = fixt!(Action, AgentValidationPkgAction);
    create_action.header.author = agent.clone();
    create_action.header.timestamp = Timestamp::now();
    create_action.header.action_seq = 10;
    let action = create_action;
    let action_hashed = HoloHashed::from_content_sync(action);
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
        .expect_retrieve_public_record()
        .times(1)
        .returning(move |_hash, _options| {
            let signed_action = signed_action.clone();
            async move {
                // TODO this line creates the problem, expects a None value
                Ok(Some((
                    Record::new(
                        signed_action,
                        RecordEntry::Present(Entry::Agent(fixt!(AgentPubKey))),
                    ),
                    CascadeSource::Local,
                )))
            }
            .boxed()
        });

    // `op` is a `DhtOp` built directly (see the module-level comment); hash it
    // and pass it to `validate_op`.
    let hashed = DhtOpHashed::from_content_sync(op);

    let validation_outcome = validate_op(hashed.as_content(), &dna_def.hash, SysValDeps::default())
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

    pub fn dna_def_hash(&self) -> HoloHashed<DnaDef> {
        DnaDefHashed::from_content_sync(self.dna_def.clone())
    }

    pub async fn sign_action(&self, action: Action) -> SignedActionHashed {
        let action_hashed = HoloHashed::from_content_sync(action);
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
        let dna_hash = self.dna_def_hash().hash;

        // Swap out the cascade so we can move it into the workflow
        let mut new_cascade = MockCascade::new();
        std::mem::swap(&mut new_cascade, &mut self.cascade);

        let cascade = Arc::new(new_cascade);

        // `self.op` is a `DhtOp` built directly (see the module-level comment);
        // hash it and pass it to `retrieve_previous_actions_for_ops` and
        // `validate_op`.
        let op = self
            .op
            .as_ref()
            .expect("No op set, invalid test case")
            .clone();
        let hashed = DhtOpHashed::from_content_sync(op);

        retrieve_previous_actions_for_ops(
            self.current_validation_dependencies.clone(),
            cascade.clone(),
            vec![hashed.clone()].into_iter(),
        )
        .await;

        validate_op(
            hashed.as_content(),
            &dna_hash,
            self.current_validation_dependencies.clone(),
        )
        .await
    }
}

/// Pair an [`Action`] with a signature to build a [`SignedAction`]. The
/// signature is paired as-is (tests use `fixt!(Signature)`), so this pairs
/// rather than cryptographically signs.
fn signed(action: Action, signature: Signature) -> SignedAction {
    SignedAction::new(action, signature)
}

/// Map a [`RecordEntry`] to the [`OpEntry`] carried by entry-bearing ops.
fn to_op_entry(entry: RecordEntry) -> OpEntry {
    match entry {
        RecordEntry::Present(e) => OpEntry::Present(e),
        RecordEntry::Hidden => OpEntry::Hidden,
        RecordEntry::NA | RecordEntry::NotStored => OpEntry::ActionOnly,
    }
}

fn test_op(previous: &SignedActionHashed) -> DhtOp {
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous.action().author().clone();
    create_action.header.action_seq = previous.action().action_seq() + 1;
    create_action.header.prev_action = Some(previous.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let action = create_action;

    ChainOp::AgentActivity(signed(action, fixt!(Signature))).into()
}
