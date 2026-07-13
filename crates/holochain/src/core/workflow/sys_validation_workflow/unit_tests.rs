use super::sys_validation_workflow;
use super::validation_deps::SysValDeps;
use super::SysValidationWorkspace;
use crate::conductor::space::TestSpace;
use crate::core::queue_consumer::TriggerReceiver;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::prelude::SignatureFixturator;
use fixt::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holo_hash::HasHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::mutations::StateMutationResult;
use holochain_types::dht_v2::ChainOp;
use holochain_types::dht_v2::DhtOp;
use holochain_types::dht_v2::DhtOpHashed;
use holochain_types::dht_v2::OpEntry;
use holochain_types::dht_v2::SignedAction;
use holochain_types::record::SignedActionHashedExt;
use holochain_types::record::WireRecordOps;
use holochain_types::wire_ops::WireOps;
use holochain_zome_types::action::AppEntryDef;
use holochain_zome_types::action::EntryType;
use holochain_zome_types::dht_v2::{Action, ActionData};
use holochain_zome_types::dna_def::{DnaDef, DnaDefHashed};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::fixt::{
    ActionFixturator, AgentValidationPkgAction, CreateAction, DnaAction,
};
use holochain_zome_types::judged::Judged;
use holochain_zome_types::record::SignedActionHashed;
use holochain_zome_types::timestamp::Timestamp;
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

    let mut dna_action = fixt!(Action, DnaAction);
    dna_action.header.author = fixt!(AgentPubKey);
    dna_action.header.timestamp = Timestamp::now();
    if let ActionData::Dna(d) = &mut dna_action.data {
        d.dna_hash = test_case.dna_hash();
    }
    let op = ChainOp::AgentActivity(SignedAction::new(dna_action, fixt!(Signature)));

    let op_hash = test_case.save_op_to_dht(op.into()).await.unwrap();

    test_case.run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    test_case.expect_app_validation_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_held_in_dht_store() {
    holochain_trace::test_run();

    let mut test_case = TestCase::new().await;

    // Previous op — a *held dependency*, not an op under validation. "Held
    // locally" means cached in the DhtStore (`save_chain_op_as_cached` →
    // `cache_chain_ops`), so `retrieve_action` finds it but it is NOT queued for
    // sys validation. (Seeding it via `save_op_to_dht`/`record_incoming_ops`
    // would put it in limbo, so sys validation would try to validate the
    // dependency itself and fetch *its* prev_action from the network.)
    let mut prev_create_action = fixt!(Action, CreateAction);
    prev_create_action.header.author = test_case.agent.clone();
    prev_create_action.header.action_seq = 10;
    *prev_create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(prev_create_action.clone()).await;
    let previous_op =
        ChainOp::AgentActivity(SignedAction::new(prev_create_action, fixt!(Signature)));
    test_case
        .save_chain_op_as_cached(previous_op)
        .await
        .unwrap();

    // Op to validate, to go in the dht database
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
    let op = ChainOp::AgentActivity(SignedAction::new(create_action, fixt!(Signature))).into();

    let op_hash = test_case.save_op_to_dht(op).await.unwrap();

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
    let mut prev_create_action = fixt!(Action, CreateAction);
    prev_create_action.header.author = test_case.agent.clone();
    prev_create_action.header.action_seq = 10;
    *prev_create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case.sign_action(prev_create_action).await;

    // Op to validate, to go in the dht database
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
    let op = ChainOp::AgentActivity(SignedAction::new(create_action, fixt!(Signature))).into();

    let op_hash = test_case.save_op_to_dht(op).await.unwrap();

    let mut network = MockHolochainP2pDnaT::default();
    let mut ops: WireRecordOps = WireRecordOps::new();
    ops.action = Some(Judged::valid(
        holochain_zome_types::dht_v2::SignedAction::new(
            previous_action.action().clone(),
            previous_action.signature.clone(),
        ),
    ));
    let response = WireOps::Record(ops);
    network
        .expect_get()
        .return_once(move |_, _, _| Ok(vec![response]));

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
    let mut validation_package_action = fixt!(Action, AgentValidationPkgAction);
    validation_package_action.header.author = test_case.agent.clone();
    validation_package_action.header.action_seq = 10;
    let previous_action = test_case.sign_action(validation_package_action).await;

    // Op to validate, to go in the dht database
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
    let op = ChainOp::AgentActivity(SignedAction::new(create_action, fixt!(Signature))).into();

    test_case.save_op_to_dht(op).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    // Just return an empty response, nothing found for the request
    let response = WireOps::Record(WireRecordOps::new());
    network
        .expect_get()
        .return_once(move |_, _, _| Ok(vec![response]));

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

    // Previous op — a *held dependency*, cached in the DhtStore (not queued for
    // sys validation). See the note in `validate_op_with_dependency_held_in_dht_store`.
    let mut validation_package_action = fixt!(Action, AgentValidationPkgAction);
    validation_package_action.header.author = test_case.agent.clone();
    validation_package_action.header.action_seq = 10;
    let previous_action = test_case
        .sign_action(validation_package_action.clone())
        .await;
    let previous_op = ChainOp::AgentActivity(SignedAction::new(
        validation_package_action,
        fixt!(Signature),
    ));
    test_case
        .save_chain_op_as_cached(previous_op)
        .await
        .unwrap();

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Action, CreateAction);
    create_action.header.author = previous_action.action().author().clone();
    create_action.header.action_seq = previous_action.action().action_seq() + 31;
    create_action.header.prev_action = Some(previous_action.as_hash().clone());
    create_action.header.timestamp = Timestamp::now();
    *create_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = ChainOp::AgentActivity(SignedAction::new(create_action, fixt!(Signature))).into();
    test_case.save_op_to_dht(op).await.unwrap();

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
    let mut create = fixt!(Action, CreateAction);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.header.author = bad_agent.clone();
    *create.entry_hash_mut().unwrap() = entry.to_hash();
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.header.action_seq = 0; // Not allowed to have a 0 seq number for a Create
    let warranted_action = test_case.sign_action(create.clone()).await;
    let warranted_op = ChainOp::CreateRecord(
        SignedAction::new(create, fixt!(Signature)),
        OpEntry::Present(entry),
    );
    test_case
        .save_chain_op_as_cached(warranted_op)
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

    let warrant_op_hash = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone())).hash;

    test_case
        .save_op_to_dht(DhtOp::from((*warrant_op).clone()))
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
        .await
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
    let mut create = fixt!(Action, CreateAction);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.header.author = bad_agent.clone();
    *create.entry_hash_mut().unwrap() = entry.to_hash();
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.header.action_seq = 0; // Not allowed to have a 0 seq number for a Create
    let warranted_action = test_case.sign_action(create).await;

    network.expect_get().return_once({
        let warranted_action = warranted_action.clone();
        move |_hash, _, _| {
            let mut ops: WireRecordOps = WireRecordOps::new();
            ops.action = Some(Judged::valid(
                holochain_zome_types::dht_v2::SignedAction::new(
                    warranted_action.action().clone(),
                    warranted_action.signature.clone(),
                ),
            ));
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

    let warrant_op_hash = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone())).hash;

    test_case
        .save_op_to_dht(DhtOp::from((*warrant_op).clone()))
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
        .await
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
    let mut create = fixt!(Action, CreateAction);
    let entry = Entry::app(SerializedBytes::default()).unwrap();
    create.header.author = good_agent.clone();
    *create.entry_hash_mut().unwrap() = entry.to_hash();
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.header.action_seq = 30;
    let valid_action = test_case.sign_action(create.clone()).await;
    let valid_op = ChainOp::CreateRecord(
        SignedAction::new(create, fixt!(Signature)),
        OpEntry::Present(entry),
    );
    let valid_op_hash = DhtOpHashed::from_content_sync(DhtOp::from(valid_op.clone())).hash;
    test_case.save_op_to_dht(valid_op.into()).await.unwrap();

    // Invalid warrant against a valid action
    let warrant_op = test_case
        .create_and_sign_warrant(
            &valid_action,
            &bad_warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();

    let warrant_op_hash = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone())).hash;

    test_case
        .save_op_to_dht(DhtOp::from((*warrant_op).clone()))
        .await
        .unwrap();

    // Validate the valid dependency and discover the warrant op in the DHT
    let work_complete = test_case.run().await;
    assert!(matches!(work_complete, WorkComplete::Incomplete(_)));

    // Check that the dependency got sys validated: it should no longer be
    // pending sys-validation in the DHT store, and must not yet carry a
    // terminal (app/integration) outcome.
    let dht_store = &test_case.test_space.space.dht_store;
    let pending = dht_store
        .as_read()
        .ops_pending_sys_validation(100)
        .await
        .unwrap();
    assert!(
        !pending.iter().any(|op| op.as_hash() == &valid_op_hash),
        "dependency op should no longer be pending sys-validation"
    );
    assert!(dht_store
        .as_read()
        .op_validation_status(
            valid_action.as_hash(),
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap()
        .is_none());

    // Mark the sys-validated dependency as valid in the DHT store (this test
    // can't run the app-validation + integration workflows), so the warrant-
    // dependency readiness check sees a terminal outcome.
    test_case.mark_dep_valid_in_store(&valid_op_hash).await;

    // Validate the warrant itself
    test_case.run().await;

    let status = test_case
        .get_warrant_validation_outcome(warrant_op_hash)
        .await
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
    let mut create = fixt!(Action, CreateAction);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.header.author = good_agent.clone();
    *create.entry_hash_mut().unwrap() = entry.to_hash();
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.header.action_seq = 30;
    let valid_action = test_case.sign_action(create.clone()).await;
    let valid_op = ChainOp::CreateRecord(
        SignedAction::new(create, fixt!(Signature)),
        OpEntry::Present(entry),
    );
    let valid_op_hash = DhtOpHashed::from_content_sync(DhtOp::from(valid_op.clone())).hash;
    test_case.save_op_to_dht(valid_op.into()).await.unwrap();
    test_case.mark_dep_valid_in_store(&valid_op_hash).await;

    // Invalid warrant against a valid action
    let warrant_op = test_case
        .create_and_sign_warrant(
            &valid_action,
            &bad_warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();

    let warrant_op_hash = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone())).hash;

    test_case
        .save_op_to_dht(DhtOp::from((*warrant_op).clone()))
        .await
        .unwrap();

    // Validate the valid dependency and discover the warrant op in the DHT
    let work_complete = test_case.run().await;
    assert!(matches!(work_complete, WorkComplete::Complete));

    // Get the warrant validation outcome
    let status = test_case
        .get_warrant_validation_outcome(warrant_op_hash)
        .await
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
    let mut create = fixt!(Action, CreateAction);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.header.author = bad_agent.clone();
    *create.entry_hash_mut().unwrap() = entry.to_hash();
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create.header.action_seq = 0; // Not allowed for a create op
    let valid_action = test_case.sign_action(create.clone()).await;
    let invalid_op = ChainOp::CreateRecord(
        SignedAction::new(create, fixt!(Signature)),
        OpEntry::Present(entry),
    );
    test_case.save_op_to_dht(invalid_op.into()).await.unwrap();

    // Valid warrant against the invalid action
    let warrant_op = test_case
        .create_and_sign_warrant(
            &valid_action,
            &warrant_agent,
            holochain_zome_types::op::ChainOpType::StoreRecord,
        )
        .await
        .unwrap();
    let warrant_op_hash = DhtOpHashed::from_content_sync(DhtOp::from((*warrant_op).clone())).hash;
    test_case
        .save_op_to_dht(DhtOp::from((*warrant_op).clone()))
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
        .await
        .unwrap();

    assert!(
        matches!(
            status,
            Some(holochain_zome_types::prelude::ValidationStatus::Valid)
        ),
        "Warrant was not rejected as expected, got: {status:?}"
    );

    // Check that no new warrant was issued
    let dht_warrants = test_case
        .test_space
        .space
        .dht_store
        .as_read()
        .warrants_by_author(other_warrant_agent.clone())
        .await
        .unwrap();
    assert_eq!(
        0,
        dht_warrants.len(),
        "No new warrant should have been issued"
    );

    // Check that the original warrant is still present
    let dht_warrants = test_case
        .test_space
        .space
        .dht_store
        .as_read()
        .warrants_by_author(warrant_agent.clone())
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

    async fn sign_action(&self, action: Action) -> SignedActionHashed {
        let action_hashed = holo_hash::HoloHashed::from_content_sync(action);
        SignedActionHashed::sign(&self.keystore, action_hashed)
            .await
            .unwrap()
    }

    fn with_network_behaviour(&mut self, network: MockHolochainP2pDnaT) -> &mut Self {
        self.actual_network = Some(network);
        self
    }

    /// Write a chain op to the DHT store as a cached (not locally-validated)
    /// row, so it is visible to `move_warranted_op_to_limbo`.
    async fn save_chain_op_as_cached(&self, chain_op: ChainOp) -> StateMutationResult<DhtOpHash> {
        let op_hash = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op.clone())))
            .as_hash()
            .clone();

        // Build the RenderedOps first so we can pass it to cache_chain_ops.
        let action = chain_op.signed_action().data().clone();
        let signature = chain_op.signed_action().signature().clone();
        let op_type = chain_op.op_type();
        let entry = match chain_op.op_entry() {
            Some(OpEntry::Present(e)) => Some(
                holochain_zome_types::entry::EntryHashed::from_content_sync(e.clone()),
            ),
            _ => None,
        };

        let rendered_op =
            holochain_types::wire_ops::RenderedOp::new(action, signature, None, op_type)
                .expect("render op for test fixture");
        let rendered_ops = holochain_types::wire_ops::RenderedOps {
            entry,
            ops: vec![rendered_op],
            warrant: None,
        };

        self.test_space
            .space
            .dht_store
            .cache_chain_ops(&rendered_ops)
            .await?;

        Ok(op_hash)
    }

    /// Write an op to the DHT store so that `ops_pending_sys_validation`
    /// returns it.
    async fn save_op_to_dht(&self, op: DhtOp) -> StateMutationResult<DhtOpHash> {
        let op_hashed = DhtOpHashed::from_content_sync(op);
        let hash = op_hashed.as_hash().clone();

        // Write to the DHT store so that `ops_pending_sys_validation` returns it.
        self.test_space
            .space
            .dht_store
            .record_incoming_ops(vec![(op_hashed, false)])
            .await
            .unwrap();

        Ok(hash)
    }

    /// Record a terminal Valid outcome for `op_hash` directly in the DHT store
    /// limbo (sys + app accepted). These unit tests run only the sys-validation
    /// workflow, so they can't drive a dependency all the way through
    /// app-validation + integration; this seeds the decided outcome the
    /// warrant-dependency readiness check (`op_validation_status`) reads.
    async fn mark_dep_valid_in_store(&self, op_hash: &DhtOpHash) {
        use holochain_state::dht_store::{AppOutcome, SysOutcome};
        self.test_space
            .space
            .dht_store
            .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        self.test_space
            .space
            .dht_store
            .record_app_validation_outcomes(vec![(op_hash.clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
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
                        reason: "test warrant".into(),
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
            self.test_space.space.dht_store.clone(),
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
        self.test_space
            .space
            .dht_store
            .as_read()
            .ops_pending_app_validation(10_000)
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

    async fn get_warrant_validation_outcome(
        &self,
        warrant_op_hash: DhtOpHash,
    ) -> holochain_state::prelude::StateQueryResult<
        Option<holochain_zome_types::prelude::ValidationStatus>,
    > {
        self.test_space
            .space
            .dht_store
            .as_read()
            .warrant_validation_status(&warrant_op_hash)
            .await
    }
}
