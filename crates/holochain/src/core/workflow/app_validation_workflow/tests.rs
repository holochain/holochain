use crate::conductor::{Conductor, ConductorHandle};
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::app_validation_workflow::{
    app_validation_workflow_inner, check_app_entry_def, put_validation_limbo,
    AppValidationWorkspace, OutcomeSummary,
};
use crate::core::workflow::sys_validation_workflow::validation_query;
use crate::core::{SysValidationError, ValidationOutcome};
use crate::sweettest::*;
use crate::test_utils::{
    host_fn_caller::*, new_invocation, new_zome_call, wait_for_integration, ConsistencyConditions,
};
use ::fixt::fixt;
use arbitrary::Arbitrary;
use hdk::hdi::test_utils::set_zome_types;
use hdk::prelude::*;
use holo_hash::{fixt::AgentPubKeyFixturator, ActionHash, AnyDhtHash, DhtOpHash, EntryHash};
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_sqlite::error::DatabaseResult;
use holochain_state::mutations::insert_op;
use holochain_state::prelude::{from_blob, StateQueryResult};
use holochain_state::query::{Store, Txn};
use holochain_state::test_utils::test_db_dir;
use holochain_state::validation_db::ValidationStage;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::{TestWasm, TestWasmPair, TestZomes};
use holochain_zome_types::fixt::{CreateFixturator, DeleteFixturator, SignatureFixturator};
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::Action;
use matches::assert_matches;
use rusqlite::{named_params, Transaction};
use std::convert::{TryFrom, TryInto};
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn main_workflow() {
    holochain_trace::test_run();

    let zomes =
        SweetInlineZomes::new(vec![], 0).integrity_function("validate", move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete { delete }) = op {
                let result = api.must_get_action(MustGetActionInput::new(
                    delete.hashed.deletes_address.clone(),
                ));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![delete
                            .hashed
                            .deletes_address
                            .clone()
                            .into()]),
                    ))
                }
            } else {
                Ok(ValidateCallbackResult::Valid)
            }
        });

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let dna_hash = dna_file.dna_hash().clone();

    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    let app_validation_workspace = Arc::new(AppValidationWorkspace::new(
        conductor
            .get_or_create_authored_db(&dna_hash, cell_id.agent_pubkey().clone())
            .unwrap(),
        conductor.get_dht_db(&dna_hash).unwrap(),
        conductor.get_dht_db_cache(&dna_hash).unwrap(),
        conductor.get_cache_db(&cell_id).await.unwrap(),
        conductor.keystore(),
        Arc::new(dna_file.dna_def().clone()),
    ));

    // check there are no ops to app validate
    // genesis entries have already been validated at this stage
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 0);

    // create op that following delete op depends on
    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: Default::default(),
    });
    let create_action = Action::Create(create);
    let dht_create_op = ChainOp::RegisterAgentActivity(fixt!(Signature), create_action.clone());
    let dht_create_op_hashed = DhtOpHashed::from_content_sync(dht_create_op);

    // create op that depends on previous create
    let mut delete = fixt!(Delete);
    delete.author = create_action.author().clone();
    delete.deletes_address = create_action.clone().to_hash();
    let dht_delete_op = ChainOp::RegisterDeletedEntryAction(fixt!(Signature), delete);
    let dht_delete_op_hash = DhtOpHash::with_data_sync(&dht_delete_op);
    let dht_delete_op_hashed = DhtOpHashed::from_content_sync(dht_delete_op);

    // insert op to validate in dht db and mark ready for app validation
    app_validation_workspace.dht_db.test_write(move |txn| {
        insert_op(txn, &dht_delete_op_hashed).unwrap();
        put_validation_limbo(txn, &dht_delete_op_hash, ValidationStage::SysValidated).unwrap();
    });

    // check delete op is now counted as op to validate
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 1);

    // run validation workflow
    // outcome should be incomplete - delete op is missing the dependent create op
    let outcome_summary = app_validation_workflow_inner(
        Arc::new(dna_hash.clone()),
        app_validation_workspace.clone(),
        conductor.raw_handle(),
        &conductor.holochain_p2p().to_dna(dna_hash.clone(), None),
        conductor
            .get_or_create_space(&dna_hash)
            .unwrap()
            .dht_query_cache,
    )
    .await
    .unwrap();
    assert_matches!(
        outcome_summary,
        OutcomeSummary {
            ops_to_validate: 1,
            validated: 0,
            accepted: 0,
            rejected: 0,
            warranted: 0,
            missing: 1,
            failed: empty_set,
        } if empty_set == HashSet::<DhtOpHash>::new()
    );

    // insert dependent create op in dht cache db
    // as cascade would do with fetched dependent ops
    app_validation_workspace.cache.test_write(move |txn| {
        insert_op(txn, &dht_create_op_hashed).unwrap();
        put_validation_limbo(txn, &dht_create_op_hashed.hash, ValidationStage::Pending).unwrap();
    });

    // there is still the 1 delete op to be validated
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 1);

    // run validation workflow
    // outcome should be complete
    let outcome_summary = app_validation_workflow_inner(
        Arc::new(dna_hash.clone()),
        app_validation_workspace.clone(),
        conductor.raw_handle(),
        &conductor.holochain_p2p().to_dna(dna_hash.clone(), None),
        conductor
            .get_or_create_space(&dna_hash)
            .unwrap()
            .dht_query_cache,
    )
    .await
    .unwrap();
    assert_matches!(
        outcome_summary,
        OutcomeSummary {
            ops_to_validate: 1,
            validated: 1,
            accepted: 1,
            rejected: 0,
            warranted: 0,
            missing: 0,
            failed: empty_set,
        } if empty_set == HashSet::<DhtOpHash>::new()
    );

    // check ops to validate is 0 now after having been validated
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 0);
}

// test that app validation validates multiple ops in one workflow run where
// one op depends on the other op
#[tokio::test(flavor = "multi_thread")]
async fn validate_ops_in_sequence_must_get_agent_activity() {
    holochain_trace::test_run();

    let agent = fixt!(AgentPubKey);

    // create op that following delete op depends on
    let create = Create {
        action_seq: 3,
        prev_action: fixt!(ActionHash),
        author: agent.clone(),
        entry_type: EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        }),
        entry_hash: fixt!(EntryHash),
        timestamp: Timestamp::now(),
        weight: Default::default(),
    };
    let create_action = Action::Create(create);
    let dht_create_op = ChainOp::RegisterAgentActivity(fixt!(Signature), create_action.clone());
    let dht_create_op_hashed = DhtOpHashed::from_content_sync(dht_create_op);
    let create_action_hash = create_action.to_hash();

    // create op that depends on previous create
    let delete = Delete {
        action_seq: 4,
        prev_action: create_action_hash.clone(),
        author: agent.clone(),
        deletes_address: create_action_hash.clone(),
        deletes_entry_address: create_action.entry_hash().unwrap().clone(),
        timestamp: Timestamp::now(),
        weight: Default::default(),
    };
    let delete_action = Action::Delete(delete);
    let dht_delete_op = ChainOp::RegisterAgentActivity(fixt!(Signature), delete_action.clone());
    let dht_delete_op_hash = DhtOpHash::with_data_sync(&dht_delete_op);
    let dht_delete_op_hashed = DhtOpHashed::from_content_sync(dht_delete_op);

    let entry_def = EntryDef::default_from_id("entry_def_id");
    let zomes = SweetInlineZomes::new(vec![entry_def.clone()], 0).integrity_function(
        "validate",
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete { delete }) = op {
                // chain filter goes from delete action until create action
                let chain_filter = ChainFilter::new(delete.hashed.content.clone().to_hash())
                    .until(delete.hashed.deletes_address.clone());
                let result = api.must_get_agent_activity(MustGetAgentActivityInput {
                    author: agent.clone(),
                    chain_filter: chain_filter.clone(),
                });
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(agent.clone(), chain_filter),
                    ))
                }
            } else {
                Ok(ValidateCallbackResult::Valid)
            }
        },
    );

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let dna_hash = dna_file.dna_hash().clone();

    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    let app_validation_workspace = Arc::new(AppValidationWorkspace::new(
        conductor
            .get_or_create_authored_db(&dna_hash, cell_id.agent_pubkey().clone())
            .unwrap(),
        conductor.get_dht_db(&dna_hash).unwrap(),
        conductor.get_dht_db_cache(&dna_hash).unwrap(),
        conductor.get_cache_db(&cell_id).await.unwrap(),
        conductor.keystore(),
        Arc::new(dna_file.dna_def().clone()),
    ));

    // check there are no ops to app validate
    // genesis entries have already been validated at this stage
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 0);

    // insert create and delete op in dht db and mark ready for app validation
    app_validation_workspace.dht_db.test_write(move |txn| {
        insert_op(txn, &dht_delete_op_hashed).unwrap();
        put_validation_limbo(txn, &dht_delete_op_hash, ValidationStage::SysValidated).unwrap();
        insert_op(txn, &dht_create_op_hashed).unwrap();
        put_validation_limbo(
            txn,
            &dht_create_op_hashed.hash,
            ValidationStage::SysValidated,
        )
        .unwrap();
    });

    // check create and delete op are now counted as ops to validate
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 2);

    // run validation workflow
    // outcome should be complete
    let outcome_summary = app_validation_workflow_inner(
        Arc::new(dna_hash.clone()),
        app_validation_workspace.clone(),
        conductor.raw_handle(),
        &conductor.holochain_p2p().to_dna(dna_hash.clone(), None),
        conductor
            .get_or_create_space(&dna_hash)
            .unwrap()
            .dht_query_cache,
    )
    .await
    .unwrap();
    assert_matches!(
        outcome_summary,
        OutcomeSummary {
            ops_to_validate: 2,
            validated: 2,
            accepted: 2,
            rejected: 0,
            warranted: 0,
            missing: 0,
            failed: empty_set,
        } if empty_set == HashSet::<DhtOpHash>::new()
    );

    // check ops to validate is also 0
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 0);
}

// test that app validation validates multiple ops in one workflow run where
// one op depends on the other op
// TODO this test only passes because actions are mistakenly written to the
// Action table in the dht database before being validated. Once that is fixed
// with issue https://github.com/holochain/holochain/issues/3724,
// this test should fail.
#[tokio::test(flavor = "multi_thread")]
async fn validate_ops_in_sequence_must_get_action() {
    holochain_trace::test_run();

    let entry_def = EntryDef::default_from_id("entry_def_id");
    let zomes = SweetInlineZomes::new(vec![entry_def.clone()], 0).integrity_function(
        "validate",
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete { delete }) = op {
                let result = api.must_get_action(MustGetActionInput::new(
                    delete.hashed.deletes_address.clone(),
                ));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![delete
                            .hashed
                            .deletes_address
                            .clone()
                            .into()]),
                    ))
                }
            } else {
                Ok(ValidateCallbackResult::Valid)
            }
        },
    );

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let dna_hash = dna_file.dna_hash().clone();

    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    let app_validation_workspace = Arc::new(AppValidationWorkspace::new(
        conductor
            .get_or_create_authored_db(&dna_hash, cell_id.agent_pubkey().clone())
            .unwrap(),
        conductor.get_dht_db(&dna_hash).unwrap(),
        conductor.get_dht_db_cache(&dna_hash).unwrap(),
        conductor.get_cache_db(&cell_id).await.unwrap(),
        conductor.keystore(),
        Arc::new(dna_file.dna_def().clone()),
    ));

    // check there are no ops to app validate
    // genesis entries have already been validated at this stage
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 0);

    // create op that following delete op depends on
    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let create_op = Action::Create(create);
    let dht_create_op = ChainOp::RegisterAgentActivity(fixt!(Signature), create_op.clone());
    let dht_create_op_hashed = DhtOpHashed::from_content_sync(dht_create_op);

    // create op that depends on previous create
    let mut delete = fixt!(Delete);
    delete.author = create_op.author().clone();
    delete.deletes_address = create_op.clone().to_hash();
    delete.deletes_entry_address = create_op.entry_hash().unwrap().clone();
    let dht_delete_op = ChainOp::RegisterDeletedEntryAction(fixt!(Signature), delete);
    let dht_delete_op_hash = DhtOpHash::with_data_sync(&dht_delete_op);
    let dht_delete_op_hashed = DhtOpHashed::from_content_sync(dht_delete_op);

    // insert create and delete op in dht db and mark ready for app validation
    app_validation_workspace.dht_db.test_write(move |txn| {
        insert_op(txn, &dht_delete_op_hashed).unwrap();
        put_validation_limbo(txn, &dht_delete_op_hash, ValidationStage::SysValidated).unwrap();
        insert_op(txn, &dht_create_op_hashed).unwrap();
        put_validation_limbo(
            txn,
            &dht_create_op_hashed.hash,
            ValidationStage::SysValidated,
        )
        .unwrap();
    });

    // check create and delete op are now counted as ops to validate
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 2);

    // run validation workflow
    // outcome should be complete
    let outcome_summary = app_validation_workflow_inner(
        Arc::new(dna_hash.clone()),
        app_validation_workspace.clone(),
        conductor.raw_handle(),
        &conductor.holochain_p2p().to_dna(dna_hash.clone(), None),
        conductor
            .get_or_create_space(&dna_hash)
            .unwrap()
            .dht_query_cache,
    )
    .await
    .unwrap();
    assert_matches!(
        outcome_summary,
        OutcomeSummary {
            ops_to_validate: 2,
            validated: 2,
            accepted: 2,
            rejected: 0,
            warranted: 0,
            missing: 0,
            failed: empty_set,
        } if empty_set == HashSet::<DhtOpHash>::new()
    );

    // check ops to validate is also 0
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_create_link_validation() {
    holochain_trace::test_run();

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Post(String);
    holochain_serial!(Post);
    app_entry!(Post);

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AppValidation]).await;

    let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;
    let apps = conductors.setup_app("posts_test", &[dna]).await.unwrap();

    let ((alice,), (bobbo,)) = apps.into_tuples();

    let alice_zome = alice.zome(TestWasm::AppValidation.coordinator_zome_name());
    let bob_zome = bobbo.zome(TestWasm::AppValidation.coordinator_zome_name());

    let post = Post("test_the_validation".to_string());

    // Alice creates posts to trigger link validations
    let _: Record = conductors[0]
        .call(&alice_zome, "create_post", post.clone())
        .await;
    let _: Record = conductors[0]
        .call(&alice_zome, "create_post", post.clone())
        .await;
    let record: Record = conductors[0]
        .call(&alice_zome, "create_post", post.clone())
        .await;

    await_consistency(Duration::from_secs(10), [&alice, &bobbo])
        .await
        .expect("Timed out waiting for consistency");

    let links: Vec<Link> = conductors[1].call(&bob_zome, "get_all_posts", ()).await;

    assert_eq!(links.len(), 3);
    assert_eq!(
        links[2].target.clone().into_action_hash().unwrap(),
        record.signed_action.hashed.hash
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn handle_error_in_op_validation() {
    holochain_trace::test_run();

    let entry_def = EntryDef::default_from_id("entry_def_id");
    let zomes = SweetInlineZomes::new(vec![entry_def], 0).integrity_function(
        "validate",
        move |_, op: Op| match op {
            Op::RegisterAgentActivity(_) => Err(InlineZomeError::TestError("kaputt".to_string())),
            _ => Ok(ValidateCallbackResult::Valid),
        },
    );

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let dna_hash = dna_file.dna_hash().clone();

    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    let app_validation_workspace = Arc::new(AppValidationWorkspace::new(
        conductor
            .get_or_create_authored_db(&dna_hash, cell_id.agent_pubkey().clone())
            .unwrap(),
        conductor.get_dht_db(&dna_hash).unwrap(),
        conductor.get_dht_db_cache(&dna_hash).unwrap(),
        conductor.get_cache_db(&cell_id).await.unwrap(),
        conductor.keystore(),
        Arc::new(dna_file.dna_def().clone()),
    ));

    // create register agent activity op that will return an error during validation
    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: Default::default(),
    });
    let create_action = Action::Create(create);
    let dht_create_op = ChainOp::RegisterAgentActivity(fixt!(Signature), create_action.clone());
    let dht_create_op_hash = DhtOpHash::with_data_sync(&dht_create_op);
    let dht_create_op_hashed = DhtOpHashed::from_content_sync(dht_create_op);

    // create another op that will be validated successfully
    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: Default::default(),
    });
    let entry = fixt!(Entry);
    let dht_store_entry_op =
        ChainOp::StoreEntry(fixt!(Signature), NewEntryAction::Create(create), entry);
    let dht_store_entry_op_hash = DhtOpHash::with_data_sync(&dht_store_entry_op);
    let dht_store_entry_op_hashed = DhtOpHashed::from_content_sync(dht_store_entry_op);

    // insert both ops in dht db and mark ready for app validation
    let expected_failed_dht_op_hash = dht_create_op_hash.clone();
    app_validation_workspace.dht_db.test_write(move |txn| {
        insert_op(txn, &dht_create_op_hashed).unwrap();
        put_validation_limbo(txn, &dht_create_op_hash, ValidationStage::SysValidated).unwrap();
        insert_op(txn, &dht_store_entry_op_hashed).unwrap();
        put_validation_limbo(txn, &dht_store_entry_op_hash, ValidationStage::SysValidated).unwrap();
    });

    // check ops are now counted as ops to validate
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 2);

    // running validation workflow should finish without errors
    // outcome summary should show 1 validated and accepted op and the error-causing op as still to validate
    // the failed op should be among the failed op hashes
    let outcome_summary = app_validation_workflow_inner(
        Arc::new(dna_hash.clone()),
        app_validation_workspace.clone(),
        conductor.raw_handle(),
        &conductor.holochain_p2p().to_dna(dna_hash.clone(), None),
        conductor
            .get_or_create_space(&dna_hash)
            .unwrap()
            .dht_query_cache,
    )
    .await
    .unwrap();
    let mut expected_failed = HashSet::new();
    expected_failed.insert(expected_failed_dht_op_hash);
    assert_matches!(
        outcome_summary,
        OutcomeSummary {
            ops_to_validate: 2,
            validated: 1,
            accepted: 1,
            missing: 0,
            warranted: 0,
            rejected: 0,
            failed: actual_failed,
        } if actual_failed == expected_failed
    );

    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap()
            .len();
    assert_eq!(ops_to_validate, 1);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "deal with the invalid data that leads to blocks being enforced"]
async fn app_validation_workflow_test() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
        TestWasm::Validate,
        TestWasm::ValidateLink,
        TestWasm::Create,
    ])
    .await;

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    let alice_cell_id = alice.cell_id().clone();
    let bob_cell_id = bob.cell_id().clone();

    conductors.exchange_peer_info().await;

    let expected_count = run_test(
        alice_cell_id.clone(),
        bob_cell_id.clone(),
        &conductors,
        &dna_file,
    )
    .await;
    run_test_entry_def_id(
        alice_cell_id,
        bob_cell_id,
        &conductors,
        &dna_file,
        expected_count,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_private_entries_are_passed_to_validation_only_when_authored_with_full_entry() {
    holochain_trace::test_run();

    #[hdk_entry_helper]
    pub struct Post(String);

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type")]
    #[hdk_entry_types(skip_hdk_extern = true)]
    #[unit_enum(UnitEntryTypes)]
    pub enum EntryTypes {
        #[entry_type(visibility = "private")]
        Post(Post),
    }

    let validation_ops = std::sync::Arc::new(parking_lot::Mutex::new(vec![]));
    let validation_ops_2 = validation_ops.clone();

    let validation_failures = std::sync::Arc::new(parking_lot::Mutex::new(vec![]));
    let validation_failures_2 = validation_failures.clone();

    let entry_def = EntryDef {
        id: "unit".into(),
        visibility: EntryVisibility::Private,
        ..Default::default()
    };

    let zomeset = InlineZomeSet::new_unique([("integrity", vec![entry_def], 0)], ["coordinator"])
        .function("integrity", "validate", move |_h, op: Op| {
            // Note, we have to be a bit aggressive about setting the HDI, since it is thread_local
            // and we're not guaranteed to be running on the same thread throughout the test.
            set_zome_types(&[(0, 3)], &[]);
            validation_ops_2.lock().push(op.clone());
            if let Err(err) = op.flattened::<EntryTypes, ()>() {
                validation_failures_2.lock().push(err);
            }
            Ok(ValidateResult::Valid)
        })
        .function("coordinator", "create", |h, ()| {
            // Note, we have to be a bit aggressive about setting the HDI, since it is thread_local
            // and we're not guaranteed to be running on the same thread throughout the test.
            set_zome_types(&[(0, 3)], &[]);
            let claim = CapClaimEntry {
                tag: "tag".into(),
                grantor: ::fixt::fixt!(AgentPubKey),
                secret: ::fixt::fixt!(CapSecret),
            };
            let input = EntryTypes::Post(Post("whatever".into()));
            let location = EntryDefLocation::app(0, 0);
            let visibility = EntryVisibility::from(&input);
            assert_eq!(visibility, EntryVisibility::Private);
            let entry = input.try_into().unwrap();
            h.create(CreateInput::new(
                location.clone(),
                visibility,
                entry,
                ChainTopOrdering::default(),
            ))?;
            h.create(CreateInput::new(
                EntryDefLocation::CapClaim,
                visibility,
                Entry::CapClaim(claim),
                ChainTopOrdering::default(),
            ))?;

            Ok(())
        });
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomeset).await;

    // Note, we have to be a bit aggressive about setting the HDI, since it is thread_local
    // and we're not guaranteed to be running on the same thread throughout the test.
    set_zome_types(&[(0, 3)], &[]);

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();

    conductors.exchange_peer_info().await;

    let () = conductors[0]
        .call(&alice.zome("coordinator"), "create", ())
        .await;

    await_consistency(30, [&alice, &bob]).await.unwrap();

    {
        let vfs = validation_failures.lock();
        if !vfs.is_empty() {
            panic!("{} validation failures encountered: {:#?}", vfs.len(), vfs);
        }
    }

    let mut num_store_entry_private = 0;
    let mut num_store_record_private = 0;
    let mut num_register_agent_activity_private = 0;

    for op in validation_ops.lock().iter() {
        match op {
            Op::StoreEntry(StoreEntry { action, entry: _ }) => {
                if *action.hashed.entry_type().visibility() == EntryVisibility::Private {
                    num_store_entry_private += 1
                }
            }
            Op::StoreRecord(StoreRecord { record }) => {
                if record
                    .action()
                    .entry_type()
                    .map(|et| *et.visibility() == EntryVisibility::Private)
                    .unwrap_or(false)
                {
                    num_store_record_private += 1
                }
                let (privatized, _) = record.clone().privatized();
                assert_eq!(record, &privatized);
            }
            Op::RegisterAgentActivity(RegisterAgentActivity {
                action,
                cached_entry: _,
            }) => {
                if action
                    .hashed
                    .entry_type()
                    .map(|et| *et.visibility() == EntryVisibility::Private)
                    .unwrap_or(false)
                {
                    num_register_agent_activity_private += 1
                }
            }
            _ => unreachable!(),
        }
    }

    // - Of the two private entries alice committed, only alice should validate these as a StoreEntry.
    // - However, both Alice and Bob should validate and integrate the StoreRecord and RegisterAgentActivity,
    //     even though the entries are private.
    assert_eq!(
        (
            num_store_entry_private,
            num_store_record_private,
            num_register_agent_activity_private
        ),
        (2, 4, 4)
    )
}

/// Check the AppEntryDef is valid for the zome and the EntryDefId and ZomeIndex are in range.
#[tokio::test(flavor = "multi_thread")]
async fn check_app_entry_def_test() {
    let mut u = unstructured_noise();
    holochain_trace::test_run();
    let TestWasmPair::<DnaWasm> {
        integrity,
        coordinator,
    } = TestWasm::EntryDefs.into();
    // Setup test data
    let dna_file = DnaFile::new(
        DnaDef {
            name: "app_entry_def_test".to_string(),
            modifiers: DnaModifiers {
                network_seed: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                origin_time: Timestamp::HOLOCHAIN_EPOCH,
                quantum_time: holochain_p2p::dht::spacetime::STANDARD_QUANTUM_TIME,
            },
            integrity_zomes: vec![TestZomes::from(TestWasm::EntryDefs).integrity.into_inner()],
            coordinator_zomes: vec![TestZomes::from(TestWasm::EntryDefs)
                .coordinator
                .into_inner()],
            lineage: Default::default(),
        },
        [integrity, coordinator],
    )
    .await;
    let dna_hash = dna_file.dna_hash().to_owned().clone();
    let mut entry_def = EntryDef::arbitrary(&mut u).unwrap();
    entry_def.visibility = EntryVisibility::Public;

    let db_dir = test_db_dir();
    let data_root_dir: DataRootPath = db_dir.path().to_path_buf().into();
    let conductor_handle = Conductor::builder()
        .with_data_root_path(data_root_dir)
        .test(&[])
        .await
        .unwrap();

    // ## Dna is missing
    let app_entry_def_0 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_0, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::DnaMissing(_))
    );

    // # Dna but no entry def in buffer
    // ## ZomeIndex out of range
    conductor_handle.register_dna(dna_file).await.unwrap();

    // ## EntryId is out of range
    let app_entry_def_1 = AppEntryDef::new(10.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_1, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryDefId(_)
        ))
    );

    let app_entry_def_2 = AppEntryDef::new(0.into(), 100.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_2, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::ZomeIndex(_)
        ))
    );

    // ## EntryId is in range for dna
    let app_entry_def_3 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_3, &dna_hash, &conductor_handle).await,
        Ok(_)
    );
    let app_entry_def_4 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Private);
    assert_matches!(
        check_app_entry_def(&app_entry_def_4, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryVisibility(_)
        ))
    );

    // ## Can get the entry from the entry def
    let app_entry_def_5 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_5, &dna_hash, &conductor_handle).await,
        Ok(_)
    );
}

/// Three agent test.
/// Alice is bypassing validation.
/// Bob and Carol are running a DNA with validation that will reject any new action authored.
/// Alice and Bob join the network, and Alice commits an invalid action.
/// Bob blocks Alice and authors a Warrant.
/// Carol joins the network, and receives Bob's warrant via gossip.
#[tokio::test(flavor = "multi_thread")]
async fn app_validation_produces_warrants() {
    holochain_trace::test_run();

    #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
    struct AppString(String);

    let string_entry_def = EntryDef::default_from_id("string");
    let zome_common = SweetInlineZomes::new(vec![string_entry_def], 0)
        .function("create_string", move |api, s: AppString| {
            let entry = Entry::app(s.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("get_agent_activity", move |api, agent_pubkey| {
            Ok(api.get_agent_activity(GetAgentActivityInput {
                agent_pubkey,
                chain_query_filter: Default::default(),
                activity_request: ActivityRequest::Full,
            })?)
        });

    let zome_sans_validation = zome_common
        .clone()
        .integrity_function("validate", move |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        });

    let zome_avec_validation = |_| {
        zome_common
            .clone()
            .integrity_function("validate", move |_api, op: Op| {
                if op.action_seq() > 3 {
                    Ok(ValidateCallbackResult::Invalid("nope".to_string()))
                } else {
                    Ok(ValidateCallbackResult::Valid)
                }
            })
    };

    let network_seed = "seed".to_string();

    let (dna_sans, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_sans_validation).await;
    let (dna_avec_1, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_avec_validation(1)).await;
    let (dna_avec_2, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_avec_validation(2)).await;

    let dna_hash = dna_sans.dna_hash();
    assert_eq!(dna_sans.dna_hash(), dna_avec_1.dna_hash());
    assert_eq!(dna_avec_1.dna_hash(), dna_avec_2.dna_hash());

    let mut conductors = SweetConductorBatch::from_standard_config(3).await;
    let (alice,) = conductors[0]
        .setup_app("test_app", [&dna_sans])
        .await
        .unwrap()
        .into_tuple();
    let (bob,) = conductors[1]
        .setup_app("test_app", [&dna_avec_1])
        .await
        .unwrap()
        .into_tuple();
    let (carol,) = conductors[2]
        .setup_app("test_app", [&dna_avec_2])
        .await
        .unwrap()
        .into_tuple();

    println!("AGENTS");
    println!("0 alice {}", alice.agent_pubkey());
    println!("1 bob   {}", bob.agent_pubkey());
    println!("2 carol {}", carol.agent_pubkey());

    conductors.exchange_peer_info().await;

    await_consistency(10, [&alice, &bob, &carol]).await.unwrap();

    conductors[2].shutdown().await;

    let invalid_action_hash: ActionHash = conductors[0]
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            AppString("entry1".into()),
        )
        .await;

    let conditions = ConsistencyConditions::from(vec![(alice.agent_pubkey().clone(), 1)]);

    await_consistency_advanced(
        10,
        conditions.clone(),
        [(&alice, false), (&bob, true), (&carol, false)],
    )
    .await
    .unwrap();

    conductors[0].shutdown().await;
    conductors[2].startup().await;

    // conductors[0].persist_dbs();
    // conductors[1].persist_dbs();
    // conductors[2].persist_dbs();

    //- Ensure that bob authored a warrant
    let alice_pubkey = alice.agent_pubkey().clone();
    conductors[1].spaces.get_all_authored_dbs(dna_hash).unwrap()[0].test_read(move |txn| {
        let store = Txn::from(&txn);

        let warrants = store
            .get_warrants_for_basis(&alice_pubkey.into(), false)
            .unwrap();
        // 3 warrants, one for each op
        assert_eq!(warrants.len(), 1);
    });

    // TODO: ensure that bob blocked alice

    await_consistency_advanced(
        10,
        conditions,
        [(&alice, false), (&bob, true), (&carol, true)],
    )
    .await
    .unwrap();

    //- Ensure that carol gets gossiped the warrant for alice from bob

    let basis: AnyLinkableHash = alice.agent_pubkey().clone().into();
    crate::assert_eq_retry_10s!(
        {
            let basis = basis.clone();
            conductors[2]
                .spaces
                .dht_db(dna_hash)
                .unwrap()
                .test_read(move |txn| {
                    let store = Txn::from(&txn);
                    store.get_warrants_for_basis(&basis, true).unwrap()
                })
                .len()
        },
        1
    );

    let activity: AgentActivity = conductors[2]
        .call(
            &carol.zome(SweetInlineZomes::COORDINATOR),
            "get_agent_activity",
            alice.agent_pubkey().clone(),
        )
        .await;

    // 1 warrant, even though there are 3 ops, because we de-dupe
    assert_eq!(activity.warrants.len(), 1);
    match &activity.warrants.first().unwrap().proof {
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author,
            action: (hash, _),
            validation_type: _,
        }) => {
            assert_eq!(action_author, alice.agent_pubkey());
            assert_eq!(*hash, invalid_action_hash);
        }
        _ => unreachable!(),
    }
}

// These are the expected invalid ops
fn expected_invalid_entry(
    txn: &Transaction,
    invalid_action_hash: &ActionHash,
    invalid_entry_hash: &AnyDhtHash,
) -> bool {
    let sql = "
        SELECT count(hash) FROM DhtOp WHERE
        type = :store_entry AND action_hash = :invalid_action_hash
            AND basis_hash = :invalid_entry_hash AND validation_status = :rejected
    ";

    let count: usize = txn
        .query_row(
            sql,
            named_params! {
                ":invalid_action_hash": invalid_action_hash,
                ":invalid_entry_hash": invalid_entry_hash,
                ":store_entry": ChainOpType::StoreEntry,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 1
}

// Now we expect an invalid link
fn expected_invalid_link(txn: &Transaction, invalid_link_hash: &ActionHash) -> bool {
    let sql = "
        SELECT count(hash) FROM DhtOp WHERE
        type = :create_link AND action_hash = :invalid_link_hash
            AND validation_status = :rejected
    ";

    let count: usize = txn
        .query_row(
            sql,
            named_params! {
                ":invalid_link_hash": invalid_link_hash,
                ":create_link": ChainOpType::RegisterAddLink,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 1
}

// Now we're trying to remove an invalid link
fn expected_invalid_remove_link(txn: &Transaction, invalid_remove_hash: &ActionHash) -> bool {
    let sql = "
        SELECT count(hash) FROM DhtOp WHERE
        (type = :delete_link AND action_hash = :invalid_remove_hash
            AND validation_status = :rejected)
    ";

    let count: usize = txn
        .query_row(
            sql,
            named_params! {
                ":invalid_remove_hash": invalid_remove_hash,
                ":delete_link": ChainOpType::RegisterRemoveLink,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 1
}

fn limbo_is_empty(txn: &Transaction) -> bool {
    let not_empty: bool = txn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE when_integrated IS NULL)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    !not_empty
}

fn show_limbo(txn: &Transaction) -> Vec<DhtOpLite> {
    txn.prepare(
        "
        SELECT DhtOp.type, Action.hash, Action.blob
        FROM DhtOp
        JOIN Action ON DhtOp.action_hash = Action.hash
        WHERE
        when_integrated IS NULL
    ",
    )
    .unwrap()
    .query_and_then([], |row| {
        let op_type: DhtOpType = row.get("type")?;
        match op_type {
            DhtOpType::Chain(op_type) => {
                let hash: ActionHash = row.get("hash")?;
                let action: SignedAction = from_blob(row.get("blob")?)?;
                Ok(ChainOpLite::from_type(op_type, hash, &action)?.into())
            }
            DhtOpType::Warrant(_) => {
                let warrant: SignedWarrant = from_blob(row.get("blob")?)?;
                Ok(warrant.into())
            }
        }
    })
    .unwrap()
    .collect::<StateQueryResult<Vec<DhtOpLite>>>()
    .unwrap()
}

fn num_valid(txn: &Transaction) -> usize {
    txn
    .query_row("SELECT COUNT(hash) FROM DhtOp WHERE when_integrated IS NOT NULL AND validation_status = :status",
            named_params!{
                ":status": ValidationStatus::Valid,
            },
            |row| row.get(0))
            .unwrap()
}

async fn run_test(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    conductors: &SweetConductorBatch,
    dna_file: &DnaFile,
) -> usize {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let invocation = new_zome_call(
        conductors[1].raw_handle().keystore(),
        &bob_cell_id,
        "always_validates",
        (),
        TestWasm::Validate,
    )
    .await
    .unwrap();
    conductors[1].call_zome(invocation).await.unwrap().unwrap();

    // Integration should have 3 ops in it
    // Plus another 16 for genesis + init
    // Plus 2 for Cap Grant
    let expected_count = 3 + 16 + 2;
    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();

    alice_db
        .read_async(move |txn| -> DatabaseResult<()> {
            // Validation should be empty
            let limbo = show_limbo(&txn);
            assert!(limbo_is_empty(&txn), "{:?}", limbo);

            assert_eq!(num_valid(&txn), expected_count);

            Ok(())
        })
        .await
        .unwrap();

    let (invalid_action_hash, invalid_entry_hash) =
        commit_invalid(&bob_cell_id, &conductors[1].raw_handle(), dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    // Integration should have 3 ops in it
    // StoreEntry should be invalid.
    // RegisterAgentActivity will be valid.
    let expected_count = 3 + expected_count;
    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    alice_db
        .read_async({
            let check_invalid_action_hash = invalid_action_hash.clone();
            let check_invalid_entry_hash = invalid_entry_hash.clone();

            move |txn| -> DatabaseResult<()> {
                // Validation should be empty
                let limbo = show_limbo(&txn);
                assert!(limbo_is_empty(&txn), "{:?}", limbo);

                assert!(expected_invalid_entry(
                    &txn,
                    &check_invalid_action_hash,
                    &check_invalid_entry_hash
                ));
                // Expect having one invalid op for the store entry.
                assert_eq!(num_valid(&txn), expected_count - 1);

                Ok(())
            }
        })
        .await
        .unwrap();

    let invocation = new_zome_call(
        conductors[1].raw_handle().keystore(),
        &bob_cell_id,
        "add_valid_link",
        (),
        TestWasm::ValidateLink,
    )
    .await
    .unwrap();
    conductors[1].call_zome(invocation).await.unwrap().unwrap();

    // Integration should have 6 ops in it
    let expected_count = 6 + expected_count;
    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    alice_db
        .read_async({
            let check_invalid_action_hash = invalid_action_hash.clone();
            let check_invalid_entry_hash = invalid_entry_hash.clone();

            move |txn| -> DatabaseResult<()> {
                // Validation should be empty
                let limbo = show_limbo(&txn);
                assert!(limbo_is_empty(&txn), "{:?}", limbo);

                assert!(expected_invalid_entry(
                    &txn,
                    &check_invalid_action_hash,
                    &check_invalid_entry_hash
                ));
                // Expect having one invalid op for the store entry.
                assert_eq!(num_valid(&txn), expected_count - 1);

                Ok(())
            }
        })
        .await
        .unwrap();

    let invocation = new_invocation(
        conductors[1].raw_handle().keystore(),
        &bob_cell_id,
        "add_invalid_link",
        (),
        TestWasm::ValidateLink.coordinator_zome(),
    )
    .await
    .unwrap();
    let invalid_link_hash: ActionHash = call_zome_directly(
        &bob_cell_id,
        &conductors[1].raw_handle(),
        dna_file,
        invocation,
    )
    .await
    .decode()
    .unwrap();

    // Integration should have 9 ops in it
    let expected_count = 9 + expected_count;
    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    alice_db
        .read_async({
            let check_invalid_action_hash = invalid_action_hash.clone();
            let check_invalid_entry_hash = invalid_entry_hash.clone();
            let check_invalid_link_hash = invalid_link_hash.clone();

            move |txn| -> DatabaseResult<()> {
                // Validation should be empty
                let limbo = show_limbo(&txn);
                assert!(limbo_is_empty(&txn), "{:?}", limbo);

                assert!(expected_invalid_entry(
                    &txn,
                    &check_invalid_action_hash,
                    &check_invalid_entry_hash
                ));
                assert!(expected_invalid_link(&txn, &check_invalid_link_hash));
                // Expect having two invalid ops for the two store entries.
                assert_eq!(num_valid(&txn), expected_count - 2);

                Ok(())
            }
        })
        .await
        .unwrap();

    let invocation = new_invocation(
        conductors[1].raw_handle().keystore(),
        &bob_cell_id,
        "remove_valid_link",
        (),
        TestWasm::ValidateLink.coordinator_zome(),
    )
    .await
    .unwrap();
    call_zome_directly(
        &bob_cell_id,
        &conductors[1].raw_handle(),
        dna_file,
        invocation,
    )
    .await;

    // Integration should have 9 ops in it
    let expected_count = 9 + expected_count;
    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    alice_db
        .read_async({
            let check_invalid_action_hash = invalid_action_hash.clone();
            let check_invalid_entry_hash = invalid_entry_hash.clone();
            let check_invalid_link_hash = invalid_link_hash.clone();

            move |txn| -> DatabaseResult<()> {
                // Validation should be empty
                let limbo = show_limbo(&txn);
                assert!(limbo_is_empty(&txn), "{:?}", limbo);

                assert!(expected_invalid_entry(
                    &txn,
                    &check_invalid_action_hash,
                    &check_invalid_entry_hash
                ));
                assert!(expected_invalid_link(&txn, &check_invalid_link_hash));
                // Expect having two invalid ops for the two store entries.
                assert_eq!(num_valid(&txn), expected_count - 2);

                Ok(())
            }
        })
        .await
        .unwrap();

    let invocation = new_invocation(
        conductors[1].raw_handle().keystore(),
        &bob_cell_id,
        "remove_invalid_link",
        (),
        TestWasm::ValidateLink.coordinator_zome(),
    )
    .await
    .unwrap();
    let invalid_remove_hash: ActionHash = call_zome_directly(
        &bob_cell_id,
        &conductors[1].raw_handle(),
        dna_file,
        invocation,
    )
    .await
    .decode()
    .unwrap();

    // Integration should have 12 ops in it
    let expected_count = 12 + expected_count;
    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    alice_db
        .read_async({
            let check_invalid_action_hash = invalid_action_hash.clone();
            let check_invalid_entry_hash = invalid_entry_hash.clone();
            let check_invalid_link_hash = invalid_link_hash.clone();

            move |txn| -> DatabaseResult<()> {
                // Validation should be empty
                let limbo = show_limbo(&txn);
                assert!(limbo_is_empty(&txn), "{:?}", limbo);

                assert!(expected_invalid_entry(
                    &txn,
                    &check_invalid_action_hash,
                    &check_invalid_entry_hash
                ));
                assert!(expected_invalid_link(&txn, &check_invalid_link_hash));
                assert!(expected_invalid_remove_link(&txn, &invalid_remove_hash));
                // 3 invalid ops above plus 1 extra invalid ops that `remove_invalid_link` commits.
                assert_eq!(num_valid(&txn), expected_count - (3 + 1));

                Ok(())
            }
        })
        .await
        .unwrap();
    expected_count
}

/// 1. Commits an entry with validate_create_entry_<EntryDefId> callback
/// 2. The callback rejects the entry proving that it actually ran.
/// 3. Reject only Post with "Banana" as the String to show it doesn't
///    affect other entries.
async fn run_test_entry_def_id(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    conductors: &SweetConductorBatch,
    dna_file: &DnaFile,
    expected_count: usize,
) {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let (invalid_action_hash, invalid_entry_hash) =
        commit_invalid_post(&bob_cell_id, &conductors[1].raw_handle(), dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    // Integration should have 3 ops in it
    // StoreEntry and StoreRecord should be invalid.
    let expected_count = 3 + expected_count;
    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    alice_db
        .read_async(move |txn| -> DatabaseResult<()> {
            // Validation should be empty
            let limbo = show_limbo(&txn);
            assert!(limbo_is_empty(&txn), "{:?}", limbo);

            assert!(expected_invalid_entry(
                &txn,
                &invalid_action_hash,
                &invalid_entry_hash
            ));
            // Expect having two invalid ops for the two store entries plus the 3 from the previous test.
            assert_eq!(num_valid(&txn), expected_count - 5);

            Ok(())
        })
        .await
        .unwrap();
}

// Need to "hack holochain" because otherwise the invalid
// commit is caught by the call zome workflow
async fn commit_invalid(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (ActionHash, EntryHash) {
    let entry = ThisWasmEntry::NeverValidates;
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data.get_entry_type(TestWasm::Validate, 0).zome_index;
    // 4
    let invalid_action_hash = call_data
        .commit_entry(
            entry.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, 0),
            EntryVisibility::Public,
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger(&"commit_invalid");
    (invalid_action_hash, entry_hash)
}

// Need to "hack holochain" because otherwise the invalid
// commit is caught by the call zome workflow
async fn commit_invalid_post(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (ActionHash, EntryHash) {
    // Bananas are not allowed
    let entry = Post("Banana".into());
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    // Create call data for the 3rd zome Create
    let call_data = HostFnCaller::create_for_zome(bob_cell_id, handle, dna_file, 2).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;
    // 9
    let invalid_action_hash = call_data
        .commit_entry(
            entry.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger(&"commit_invalid_post");
    (invalid_action_hash, entry_hash)
}

async fn call_zome_directly(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
    invocation: ZomeCallInvocation,
) -> ExternIO {
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    // 4
    let output = call_data.call_zome_direct(invocation).await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger(&"call_zome_directly");
    output
}
