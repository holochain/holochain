use super::sys_validation_workflow;
use super::validation_deps::SysValDeps;
use super::validation_query::get_ops_to_app_validate;
use super::SysValidationWorkspace;
use crate::conductor::space::TestSpace;
use crate::core::queue_consumer::TriggerReceiver;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::prelude::AgentValidationPkgFixturator;
use crate::prelude::CreateFixturator;
use crate::prelude::SignatureFixturator;
use fixt::*;
use hdk::prelude::Dna as HdkDna;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holo_hash::HasHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::db::DbKindT;
use holochain_sqlite::db::DbWrite;
use holochain_state::mutations::StateMutationResult;
use holochain_types::dht_op::ChainOp;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::WireOps;
use holochain_types::record::SignedActionHashedExt;
use holochain_types::record::WireRecordOps;
use holochain_zome_types::action::ActionHashed;
use holochain_zome_types::action::AppEntryDef;
use holochain_zome_types::action::EntryType;
use holochain_zome_types::dna_def::{DnaDef, DnaDefHashed};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::judged::Judged;
use holochain_zome_types::record::SignedActionHashed;
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::Action;
use std::collections::HashSet;
use std::sync::Arc;
use {
    hdk::prelude::AppEntryBytesFixturator, holo_hash::HashableContentExtSync,
    holochain_serialized_bytes::SerializedBytes, holochain_zome_types::Entry,
};

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_no_dependency() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    let mut network = MockHolochainP2pDnaT::default();
    network
        .expect_target_arcs()
        .return_once(|| Ok(vec![kitsune2_api::DhtArc::Empty]));
    test_case.actual_network = Some(network);

    let dna_action = HdkDna {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now(),
        hash: test_case.dna_hash(),
    };
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action));

    let op_hash = test_case
        .save_op_to_db(test_case.dht_db_handle(), op.into())
        .await
        .unwrap();

    test_case.run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    test_case.expect_app_validation_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_held_in_cache() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous op, to go in the cache
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone();
    prev_create_action.action_seq = 10;
    prev_create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action.clone()))
        .await;
    let previous_op =
        ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(prev_create_action)).into();
    test_case
        .save_op_to_db(test_case.cache_db_handle(), previous_op)
        .await
        .unwrap();

    // Op to validate, to go in the dht database
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

    let op_hash = test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    let mut network = MockHolochainP2pDnaT::default();
    network
        .expect_target_arcs()
        .return_once(|| Ok(vec![kitsune2_api::DhtArc::Empty]));
    test_case.with_network_behaviour(network);

    test_case.run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    test_case.expect_app_validation_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_not_held() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous op, to be fetched from the network
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone();
    prev_create_action.action_seq = 10;
    prev_create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action.clone()))
        .await;

    // Op to validate, to go in the dht database
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

    let op_hash = test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    let mut network = MockHolochainP2pDnaT::default();
    let mut ops: WireRecordOps = WireRecordOps::new();
    ops.action = Some(Judged::valid(previous_action.clone().into()));
    let response = WireOps::Record(ops);
    network
        .expect_get()
        .return_once(move |_, _| Ok(vec![response]));

    network
        .expect_target_arcs()
        .return_once(|| Ok(vec![kitsune2_api::DhtArc::Empty]));

    test_case.with_network_behaviour(network).run().await;

    let mut network = MockHolochainP2pDnaT::default();
    network
        .expect_target_arcs()
        .return_once(|| Ok(vec![kitsune2_api::DhtArc::Empty]));

    test_case.with_network_behaviour(network).run().await;
    test_case.check_trigger_and_rerun().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    println!("Starting expectation");

    test_case.expect_app_validation_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_not_found_on_the_dht() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous op, to be referenced but not found on the dht
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(
            validation_package_action.clone(),
        ))
        .await;

    // Op to validate, to go in the dht database
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
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    // Just return an empty response, nothing found for the request
    let response = WireOps::Record(WireRecordOps::new());
    network
        .expect_get()
        .return_once(move |_, _| Ok(vec![response]));

    network
        .expect_target_arcs()
        .return_once(|| Ok(vec![kitsune2_api::DhtArc::Empty]));

    test_case.with_network_behaviour(network).run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.is_empty());

    test_case.expect_app_validation_not_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_wrong_sequence_number_rejected_and_not_forwarded_to_app_validation() {
    holochain_trace::test_run();

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_target_arcs()
        .return_once(move || Ok(vec![kitsune2_api::DhtArc::FULL]));

    let mut test_case = TestCase::new().await;
    test_case.with_network_behaviour(network);

    // Previous op, to be found in the cache
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(
            validation_package_action.clone(),
        ))
        .await;
    let previous_op = ChainOp::RegisterAgentActivity(
        fixt!(Signature),
        Action::AgentValidationPkg(validation_package_action),
    )
    .into();
    test_case
        .save_op_to_db(test_case.cache_db_handle(), previous_op)
        .await
        .unwrap();

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 31;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action)).into();
    test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    test_case.run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.is_empty());

    test_case.expect_app_validation_not_triggered().await;
}

/// Happy path test where the warranted op has already been fetched into the cache database.
/// It will have to be copied to the DHT and validated. The warrant will then be seen as valid.
#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_warrant_with_cached_dependency() {
    holochain_trace::test_run();

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_target_arcs()
        .return_once(move || Ok(vec![kitsune2_api::DhtArc::FULL]));

    let mut test_case = TestCase::new().await;
    test_case.with_network_behaviour(network);

    let bad_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();
    let warrant_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();

    // Warranted op, to be found in the cache
    let mut create = fixt!(Create);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.author = bad_agent.clone();
    create.entry_hash = entry.to_hash();
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.action_seq = 0; // Not allowed to have a 0 seq number for a Create
    let warranted_action = test_case.sign_action(Action::Create(create.clone())).await;
    let warranted_op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create),
        crate::prelude::RecordEntry::Present(entry),
    )
    .into();
    test_case
        .save_op_to_db(test_case.cache_db_handle(), warranted_op)
        .await
        .unwrap();

    let warrant_op = test_case
        .create_and_sign_warrant(
            &warranted_action,
            &warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();

    let warrant_op_hash = DhtOpHashed::from_content_sync(warrant_op.clone()).hash;

    test_case
        .save_op_to_db(
            test_case.dht_db_handle(),
            DhtOp::WarrantOp(warrant_op.into()),
        )
        .await
        .unwrap();

    // Discover the warranted op in the cache and copy it to the DHT
    test_case.run().await;

    // Validate the dependency
    let work_complete = test_case.check_trigger_and_rerun().await;
    // The warrant dep might need to go to app validation, so WorkIncomplete is about the best we
    // can really do here. It means it will take a bit of time to validate the warrant itself.
    assert!(matches!(work_complete, WorkComplete::Incomplete(_)));

    // Validate the warrant itself
    test_case.run().await;

    let status = test_case
        .get_warrant_validation_outcome(warrant_op_hash)
        .unwrap();

    assert!(
        matches!(
            status,
            Some(holochain_zome_types::prelude::ValidationStatus::Valid)
        ),
        "Warrant was not valid as expected, got: {status:?}"
    );
}

/// Happy path test where the warranted op has to be fetched from the network and cached. It will
/// have to be copied to the DHT and validated. The warrant will then be seen as valid.
#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_warrant_with_fetched_dependency() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_target_arcs()
        .return_once(move || Ok(vec![kitsune2_api::DhtArc::FULL]));

    let bad_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();
    let warrant_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();

    // Warranted op, to be fetched from the network
    let mut create = fixt!(Create);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.author = bad_agent.clone();
    create.entry_hash = entry.to_hash();
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.action_seq = 0; // Not allowed to have a 0 seq number for a Create
    let warranted_action = test_case.sign_action(Action::Create(create.clone())).await;

    network.expect_get().return_once({
        let warranted_action = warranted_action.clone();
        move |_hash, _| {
            let mut ops: WireRecordOps = WireRecordOps::new();
            ops.action = Some(Judged::valid(warranted_action.clone().into()));
            ops.entry = Some(entry);
            let response = WireOps::Record(ops);
            Ok(vec![response])
        }
    });

    test_case.with_network_behaviour(network);

    let warrant_op = test_case
        .create_and_sign_warrant(
            &warranted_action,
            &warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();

    let warrant_op_hash = DhtOpHashed::from_content_sync(warrant_op.clone()).hash;

    test_case
        .save_op_to_db(
            test_case.dht_db_handle(),
            DhtOp::WarrantOp(warrant_op.into()),
        )
        .await
        .unwrap();

    // Discover the warrant op dependency and fetch it from the network
    test_case.run().await;

    // Copy the warrant dependency to the DHT
    let work_complete = test_case.check_trigger_and_rerun().await;
    assert!(matches!(work_complete, WorkComplete::Incomplete(_)));

    // Validate the dependency
    let work_complete = test_case.check_trigger_and_rerun().await;
    assert!(matches!(work_complete, WorkComplete::Incomplete(_)));

    // Validate the warrant itself
    test_case.run().await;

    let status = test_case
        .get_warrant_validation_outcome(warrant_op_hash)
        .unwrap();

    assert!(
        matches!(
            status,
            Some(holochain_zome_types::prelude::ValidationStatus::Valid)
        ),
        "Warrant was not valid as expected, got: {status:?}"
    );
}

/// Invalid warrant test. A valid DHT op is put in the DHT database, then a warrant is issued for
/// it. Once the op is judged valid, the warrant is judged invalid and rejected.
#[tokio::test(flavor = "multi_thread")]
async fn reject_invalid_warrant() {
    holochain_trace::test_run();

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_target_arcs()
        .return_once(move || Ok(vec![kitsune2_api::DhtArc::FULL]));

    let mut test_case = TestCase::new().await;
    test_case.with_network_behaviour(network);

    let good_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();
    let bad_warrant_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();

    // Valid op, to be found in the DHT database
    let mut create = fixt!(Create);
    let entry = Entry::app(SerializedBytes::default()).unwrap();
    create.author = good_agent.clone();
    create.entry_hash = entry.to_hash();
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.action_seq = 30;
    let valid_action = test_case.sign_action(Action::Create(create.clone())).await;
    let valid_op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create),
        crate::prelude::RecordEntry::Present(entry),
    );
    let valid_op_hash = DhtOpHashed::from_content_sync(valid_op.clone()).hash;
    test_case
        .save_op_to_db(test_case.dht_db_handle(), valid_op.into())
        .await
        .unwrap();

    // Invalid warrant against a valid action
    let warrant_op = test_case
        .create_and_sign_warrant(
            &valid_action,
            &bad_warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();

    let warrant_op_hash = DhtOpHashed::from_content_sync(warrant_op.clone()).hash;

    test_case
        .save_op_to_db(
            test_case.dht_db_handle(),
            DhtOp::WarrantOp(warrant_op.into()),
        )
        .await
        .unwrap();

    // Validate the valid dependency and discover the warrant op in the DHT
    let work_complete = test_case.run().await;
    assert!(matches!(work_complete, WorkComplete::Incomplete(_)));

    // Check that the dependency got sys validated
    let (stage, state) = holochain_state::validation_db::get_dht_op_validation_state(
        &test_case.dht_db_handle().into(),
        valid_action.as_hash().clone(),
        holochain_zome_types::op::ChainOpType::StoreRecord,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(matches!(
        stage,
        Some(holochain_state::validation_db::ValidationStage::SysValidated)
    ));
    assert!(state.is_none());

    // Mark the sys validated dependency as valid because this test can't run the app validation workflow.
    test_case
        .dht_db_handle()
        .write_async(move |txn| -> StateMutationResult<()> {
            holochain_state::mutations::set_validation_status(
                txn,
                &valid_op_hash,
                holochain_zome_types::prelude::ValidationStatus::Valid,
            )?;
            holochain_state::mutations::set_when_integrated(txn, &valid_op_hash, Timestamp::now())?;
            Ok(())
        })
        .await
        .unwrap();

    // Validate the warrant itself
    test_case.run().await;

    let status = test_case
        .get_warrant_validation_outcome(warrant_op_hash)
        .unwrap();

    assert!(
        matches!(
            status,
            Some(holochain_zome_types::prelude::ValidationStatus::Rejected)
        ),
        "Warrant was not rejected as expected, got: {status:?}"
    );
}

/// Checks that if the dependency of a warrant is available locally and has already been validated,
/// then the warrant can be validated straight away without needing to process the dependency first.
#[tokio::test(flavor = "multi_thread")]
async fn validate_warrant_with_validated_dependency() {
    holochain_trace::test_run();

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_target_arcs()
        .return_once(move || Ok(vec![kitsune2_api::DhtArc::FULL]));

    let mut test_case = TestCase::new().await;
    test_case.with_network_behaviour(network);

    let good_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();
    let bad_warrant_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();

    // Valid op, to be found in the DHT database
    let mut create = fixt!(Create);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.author = good_agent.clone();
    create.entry_hash = entry.to_hash();
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.action_seq = 30;
    let valid_action = test_case.sign_action(Action::Create(create.clone())).await;
    let valid_op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create),
        crate::prelude::RecordEntry::Present(entry),
    );
    let valid_op_hash = DhtOpHashed::from_content_sync(valid_op.clone()).hash;
    test_case
        .save_op_to_db(test_case.dht_db_handle(), valid_op.into())
        .await
        .unwrap();
    test_case
        .test_space
        .space
        .dht_db
        .test_write(move |txn| -> StateMutationResult<()> {
            holochain_state::mutations::set_validation_status(
                txn,
                &valid_op_hash,
                holochain_zome_types::prelude::ValidationStatus::Valid,
            )?;
            Ok(())
        })
        .unwrap();

    // Invalid warrant against a valid action
    let warrant_op = test_case
        .create_and_sign_warrant(
            &valid_action,
            &bad_warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();

    let warrant_op_hash = DhtOpHashed::from_content_sync(warrant_op.clone()).hash;

    test_case
        .save_op_to_db(
            test_case.dht_db_handle(),
            DhtOp::WarrantOp(warrant_op.into()),
        )
        .await
        .unwrap();

    // Validate the valid dependency and discover the warrant op in the DHT
    let work_complete = test_case.run().await;
    assert!(matches!(work_complete, WorkComplete::Complete));

    // Get the warrant validation outcome
    let status = test_case
        .get_warrant_validation_outcome(warrant_op_hash)
        .unwrap();

    assert!(
        matches!(
            status,
            Some(holochain_zome_types::prelude::ValidationStatus::Rejected)
        ),
        "Warrant was not rejected as expected, got: {status:?}"
    );
}

/// Checks that validating a warranted op does not result in issuing a new warrant.
#[tokio::test(flavor = "multi_thread")]
async fn avoid_duplicate_warrant() {
    holochain_trace::test_run();

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_target_arcs()
        .return_once(move || Ok(vec![kitsune2_api::DhtArc::FULL]));

    let mut test_case = TestCase::new().await;
    test_case.with_network_behaviour(network);

    let bad_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();
    let warrant_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();
    let other_warrant_agent = test_case.keystore.new_sign_keypair_random().await.unwrap();

    // Invalid op
    let mut create = fixt!(Create);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.author = bad_agent.clone();
    create.entry_hash = entry.to_hash();
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.action_seq = 0; // Not allowed for a create op
    let valid_action = test_case.sign_action(Action::Create(create.clone())).await;
    let invalid_op = ChainOp::StoreRecord(
        fixt!(Signature),
        Action::Create(create),
        crate::prelude::RecordEntry::Present(entry),
    );
    test_case
        .save_op_to_db(test_case.dht_db_handle(), invalid_op.into())
        .await
        .unwrap();

    // Valid warrant against the invalid action
    let warrant_op = test_case
        .create_and_sign_warrant(
            &valid_action,
            &warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();
    let warrant_op_hash = DhtOpHashed::from_content_sync(warrant_op.clone()).hash;
    test_case
        .save_op_to_db(
            test_case.dht_db_handle(),
            DhtOp::WarrantOp(warrant_op.into()),
        )
        .await
        .unwrap();

    // Validate the warrant as another agent
    let work_complete = test_case.run_as_agent(&other_warrant_agent).await;
    assert!(matches!(work_complete, WorkComplete::Incomplete(_)));

    let work_complete = test_case.run_as_agent(&other_warrant_agent).await;
    assert!(matches!(work_complete, WorkComplete::Complete));

    // Get the warrant validation outcome
    let status = test_case
        .get_warrant_validation_outcome(warrant_op_hash)
        .unwrap();

    assert!(
        matches!(
            status,
            Some(holochain_zome_types::prelude::ValidationStatus::Valid)
        ),
        "Warrant was not rejected as expected, got: {status:?}"
    );

    // Check that no new warrant was issued
    let authored_warrants = test_case
        .get_authored_warrants(
            &test_case
                .test_space
                .space
                .get_or_create_authored_db(other_warrant_agent.clone())
                .unwrap(),
            other_warrant_agent.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        0,
        authored_warrants.len(),
        "No new warrant should have been issued"
    );

    let dht_warrants = test_case
        .get_authored_warrants(&test_case.dht_db_handle(), other_warrant_agent.clone())
        .await
        .unwrap();
    assert_eq!(
        0,
        dht_warrants.len(),
        "No new warrant should have been issued"
    );

    // Check that the original warrant is still present
    let dht_warrants = test_case
        .get_authored_warrants(&test_case.dht_db_handle(), warrant_agent.clone())
        .await
        .unwrap();
    assert_eq!(
        1,
        dht_warrants.len(),
        "The original warrant should still be present"
    );
}

struct TestCase {
    dna_hash: DnaDefHashed,
    test_space: TestSpace,
    keystore: MetaLairClient,
    agent: AgentPubKey,
    current_validation_dependencies: SysValDeps,
    app_validation_trigger: (TriggerSender, TriggerReceiver),
    integration_trigger: (TriggerSender, TriggerReceiver),
    publish_trigger: (TriggerSender, TriggerReceiver),
    self_trigger: (TriggerSender, TriggerReceiver),
    actual_network: Option<MockHolochainP2pDnaT>,
}

impl TestCase {
    async fn new() -> Self {
        let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
        let dna_hash = DnaDefHashed::from_content_sync(dna_def.clone());

        let test_space = TestSpace::new(dna_hash.hash.clone());

        let keystore = holochain_keystore::test_keystore();
        let agent = keystore.new_sign_keypair_random().await.unwrap();

        Self {
            dna_hash,
            test_space,
            keystore,
            agent,
            current_validation_dependencies: SysValDeps::default(),
            app_validation_trigger: TriggerSender::new(),
            integration_trigger: TriggerSender::new(),
            publish_trigger: TriggerSender::new(),
            self_trigger: TriggerSender::new(),
            actual_network: None,
        }
    }

    fn dna_hash(&self) -> DnaHash {
        self.dna_hash.hash.clone()
    }

    fn dht_db_handle(&self) -> DbWrite<DbKindDht> {
        self.test_space.space.dht_db.clone()
    }

    fn cache_db_handle(&self) -> DbWrite<DbKindCache> {
        self.test_space.space.cache_db.clone()
    }

    async fn sign_action(&self, action: Action) -> SignedActionHashed {
        let action_hashed = ActionHashed::from_content_sync(action);
        SignedActionHashed::sign(&self.keystore, action_hashed)
            .await
            .unwrap()
    }

    fn with_network_behaviour(&mut self, network: MockHolochainP2pDnaT) -> &mut Self {
        self.actual_network = Some(network);
        self
    }

    async fn save_op_to_db<T: DbKindT>(
        &self,
        db: DbWrite<T>,
        op: DhtOp,
    ) -> StateMutationResult<DhtOpHash> {
        let op = DhtOpHashed::from_content_sync(op);

        let test_op_hash = op.as_hash().clone();
        db.write_async({
            move |txn| -> StateMutationResult<()> {
                holochain_state::mutations::insert_op_untyped(txn, &op, 0)?;
                Ok(())
            }
        })
        .await
        .unwrap();

        Ok(test_op_hash)
    }

    async fn create_and_sign_warrant(
        &self,
        warranted_action: &SignedActionHashed,
        issuing_agent: &AgentPubKey,
        chain_op_type: holochain_zome_types::op::ChainOpType,
    ) -> lair_keystore_api::LairResult<holochain_types::prelude::WarrantOp> {
        holochain_types::prelude::WarrantOp::sign(
            &self.keystore,
            holochain_zome_types::prelude::Warrant::new(
                holochain_zome_types::prelude::WarrantProof::ChainIntegrity(
                    holochain_zome_types::prelude::ChainIntegrityWarrant::InvalidChainOp {
                        action_author: warranted_action.action().author().clone(),
                        action: (
                            warranted_action.as_hash().clone(),
                            warranted_action.signature.clone(),
                        ),
                        chain_op_type,
                    },
                ),
                issuing_agent.clone(),
                Timestamp::now(),
                warranted_action.action().author().clone(),
            ),
        )
        .await
    }

    async fn run(&mut self) -> WorkComplete {
        self.run_as_agent(&self.agent.clone()).await
    }

    async fn run_as_agent(&mut self, agent: &AgentPubKey) -> WorkComplete {
        let workspace = SysValidationWorkspace::new(
            self.test_space
                .space
                .get_or_create_authored_db(agent.clone())
                .unwrap(),
            self.test_space.space.dht_db.clone(),
            self.test_space.space.cache_db.clone(),
            self.dna_hash.hash.clone(),
            std::time::Duration::from_secs(10),
        );

        println!("Running with network: {:?}", self.actual_network);
        let actual_network = Arc::new(self.actual_network.take().unwrap_or_default());

        sys_validation_workflow(
            Arc::new(workspace),
            self.current_validation_dependencies.clone(),
            self.app_validation_trigger.0.clone(),
            self.integration_trigger.0.clone(),
            self.publish_trigger.0.clone(),
            self.self_trigger.0.clone(),
            actual_network,
            self.keystore.clone(),
            agent.clone(),
        )
        .await
        .unwrap()
    }

    async fn check_trigger_and_rerun(&mut self) -> WorkComplete {
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.self_trigger.1.listen(),
        )
        .await
        .unwrap()
        .unwrap();

        println!("Got a trigger, running once");

        self.run().await
    }

    /// This provides a quick and reliable way to check that ops have been sys validated
    async fn get_ops_pending_app_validation(&self) -> HashSet<DhtOpHash> {
        get_ops_to_app_validate(&self.dht_db_handle().into())
            .await
            .unwrap()
            .into_iter()
            .map(|op_hashed| op_hashed.hash)
            .collect()
    }

    async fn expect_app_validation_triggered(&mut self) {
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.app_validation_trigger.1.listen(),
        )
        .await
        .expect("Timed out waiting for app validation to be triggered")
        .unwrap();
    }

    async fn expect_app_validation_not_triggered(&mut self) {
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(1),
            self.app_validation_trigger.1.listen(),
        )
        .await
        .err()
        .is_some());
    }

    fn get_warrant_validation_outcome(
        &self,
        warrant_op_hash: DhtOpHash,
    ) -> holochain_state::prelude::StateQueryResult<
        Option<holochain_zome_types::prelude::ValidationStatus>,
    > {
        self.dht_db_handle().test_read(
            move |txn| -> holochain_state::prelude::StateQueryResult<
                Option<holochain_zome_types::prelude::ValidationStatus>,
            > {
                let status = txn.query_row(
                    r#"
            SELECT
              DhtOp.validation_status
            FROM
              DhtOp
              JOIN Warrant ON DhtOp.action_hash = Warrant.hash
            WHERE
              DhtOp.hash = :hash
            "#,
                    rusqlite::named_params! {
                        ":hash": &warrant_op_hash,
                    },
                    |row| row.get::<_, Option<holochain_zome_types::prelude::ValidationStatus>>(0),
                )?;

                Ok(status)
            },
        )
    }

    async fn get_authored_warrants<T: DbKindT>(
        &self,
        db: &holochain_sqlite::prelude::DbRead<T>,
        author: AgentPubKey,
    ) -> holochain_state::prelude::StateQueryResult<Vec<holochain_types::warrant::WarrantOp>> {
        db.read_async(
            move |txn| -> holochain_state::prelude::StateQueryResult<
                Vec<holochain_types::warrant::WarrantOp>,
            > {
                let mut stmt = txn.prepare(
                    r#"
            SELECT
                Warrant.blob as action_blob,
                Warrant.author as author,
                NULL as entry_blob,
                DhtOp.type as dht_type,
                DhtOp.hash as dht_hash,
                DhtOp.num_validation_attempts,
                DhtOp.op_order
            FROM DhtOp
                JOIN Warrant ON DhtOp.action_hash = Warrant.hash
            WHERE
                Warrant.author = :author
            "#,
                )?;

                let mut rows = stmt.query(rusqlite::named_params! {
                    ":author": author,
                })?;

                let mut ops = Vec::new();
                while let Ok(Some(row)) = rows.next() {
                    let op = holochain_state::query::map_sql_dht_op(true, "dht_type", row)?;
                    let hash = row.get("dht_hash")?;
                    ops.push(DhtOpHashed::with_pre_hashed(op, hash))
                }

                Ok(ops
                    .into_iter()
                    .filter_map(|o| match o.content {
                        DhtOp::WarrantOp(warrant_op) => Some(*warrant_op),
                        _ => None,
                    })
                    .collect())
            },
        )
        .await
    }
}
