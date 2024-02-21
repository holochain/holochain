use crate::conductor::{Conductor, ConductorHandle};
use crate::core::queue_consumer::WorkComplete;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::app_validation_workflow::{
    app_validation_workflow_inner, check_app_entry_def, put_validation_limbo,
    AppValidationWorkspace,
};
use crate::core::workflow::sys_validation_workflow::validation_query;
use crate::core::{SysValidationError, ValidationOutcome};
use crate::sweettest::{
    SweetConductor, SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetLocalRendezvous,
};
use crate::test_utils::{
    consistency_10s, host_fn_caller::*, new_invocation, new_zome_call, wait_for_integration,
};
use ::fixt::fixt;
use arbitrary::Arbitrary;
use hdk::hdi::test_utils::set_zome_types;
use hdk::prelude::*;
use holo_hash::{fixt::AgentPubKeyFixturator, ActionHash, AnyDhtHash, DhtOpHash, EntryHash};
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseResult;
use holochain_state::mutations::insert_op;
use holochain_state::prelude::{from_blob, StateQueryResult};
use holochain_state::test_utils::test_db_dir;
use holochain_state::validation_db::ValidationStage;
use holochain_types::dht_op::{DhtOp, DhtOpHashed};
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::{TestWasm, TestWasmPair, TestZomes};
use holochain_zome_types::action::Dna;
use holochain_zome_types::fixt::SignatureFixturator;
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::Action;
use matches::assert_matches;
use rusqlite::params;
use rusqlite::{named_params, Transaction};
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
#[tokio::test(flavor = "multi_thread")]
async fn main_loop_app_validation_workflow() {
    holochain_trace::test_run().unwrap();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Validate]).await;
    let dna_hash = dna_file.dna_hash().clone();

    let mut conductor = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        SweetLocalRendezvous::new().await,
    )
    .await;
    let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    let app_validation_workspace = Arc::new(AppValidationWorkspace::new(
        conductor.get_authored_db(&dna_hash).unwrap().into(),
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
            .unwrap();
    assert_eq!(ops_to_validate.len(), 0);

    // create ops to validate
    let action = Action::Dna(Dna {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now(),
        hash: dna_hash.clone(),
    });
    let dht_op = DhtOp::RegisterAgentActivity(fixt!(Signature), action.clone());
    let dht_op_hash = DhtOpHash::with_data_sync(&dht_op);
    let dht_op_hashed = DhtOpHashed::with_pre_hashed(dht_op, dht_op_hash.clone());

    let dht_op_2 = DhtOp::StoreEntry(fixt!(Signature), fixt!(NewEntryAction), fixt!(Entry));
    let dht_op_hash_2 = DhtOpHash::with_data_sync(&dht_op_2);
    let dht_op_hashed_2 = DhtOpHashed::with_pre_hashed(dht_op_2, dht_op_hash_2.clone());

    app_validation_workspace
        .dht_db
        .write_async({
            let dht_op_hash = dht_op_hash.clone();
            let dht_op_hash_2 = dht_op_hash_2.clone();
            move |txn| {
                insert_op(txn, &dht_op_hashed).unwrap();
                put_validation_limbo(txn, &dht_op_hash, ValidationStage::SysValidated).unwrap();
                insert_op(txn, &dht_op_hashed_2).unwrap();
                put_validation_limbo(txn, &dht_op_hash_2, ValidationStage::SysValidated)
            }
        })
        .await
        .unwrap();

    // check there is one op to validate now
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap();
    assert_eq!(ops_to_validate.len(), 2);

    // run validation workflow
    // outcome should be complete - all ops should have been validated
    let app_validation_result = app_validation_workflow_inner(
        Arc::new(dna_hash.clone()),
        app_validation_workspace.clone(),
        conductor.raw_handle(),
        &conductor.holochain_p2p().to_dna(dna_hash.clone(), None),
        conductor
            .get_or_create_space(&dna_hash)
            .unwrap()
            .dht_query_cache,
    )
    .await;
    assert_matches!(app_validation_result, Ok(WorkComplete::Complete));

    // check that previously inserted ops have been validated
    // and are valid
    let num_pending_ops: usize = app_validation_workspace
        .dht_db
        .read_async(move |txn| {
            txn.query_row(
                &format!(
                    "SELECT count(*)
                    from DhtOp
                    WHERE validation_stage = NULL
                    AND validation_status = 0
                    AND (
                        hash = ?1
                        OR hash = ?2
                    )"
                ),
                params![dht_op_hash, dht_op_hash_2],
                |row| row.get(0),
            )
            .map_err(DatabaseError::SqliteError)
        })
        .await
        .unwrap();
    assert_eq!(num_pending_ops, 0);

    // check ops to validate is also 0
    let ops_to_validate =
        validation_query::get_ops_to_app_validate(&app_validation_workspace.dht_db)
            .await
            .unwrap();
    assert_eq!(ops_to_validate.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "deal with the invalid data that leads to blocks being enforced"]
async fn app_validation_workflow_test() {
    holochain_trace::test_run().ok();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
        TestWasm::Validate,
        TestWasm::ValidateLink,
        TestWasm::Create,
    ])
    .await;

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let apps = conductors
        .setup_app(&"test_app", [&dna_file])
        .await
        .unwrap();
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
    holochain_trace::test_run().ok();

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
            dbg!();
            h.create(CreateInput::new(
                location.clone(),
                visibility,
                entry,
                ChainTopOrdering::default(),
            ))?;
            dbg!();
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
    let apps = conductors
        .setup_app(&"test_app", [&dna_file])
        .await
        .unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();

    conductors.exchange_peer_info().await;

    let () = conductors[0]
        .call(&alice.zome("coordinator"), "create", ())
        .await;

    consistency_10s([&alice, &bob]).await;

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
    holochain_trace::test_run().ok();
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

const SELECT: &'static str = "SELECT count(hash) FROM DhtOp WHERE";

// These are the expected invalid ops
fn expected_invalid_entry(
    txn: &Transaction,
    invalid_action_hash: &ActionHash,
    invalid_entry_hash: &AnyDhtHash,
) -> bool {
    let sql = format!(
        "
        {}
        type = :store_entry AND action_hash = :invalid_action_hash
            AND basis_hash = :invalid_entry_hash AND validation_status = :rejected
    ",
        SELECT
    );

    let count: usize = txn
        .query_row(
            &sql,
            named_params! {
                ":invalid_action_hash": invalid_action_hash,
                ":invalid_entry_hash": invalid_entry_hash,
                ":store_entry": DhtOpType::StoreEntry,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 1
}

// Now we expect an invalid link
fn expected_invalid_link(txn: &Transaction, invalid_link_hash: &ActionHash) -> bool {
    let sql = format!(
        "
        {}
        type = :create_link AND action_hash = :invalid_link_hash
            AND validation_status = :rejected
    ",
        SELECT
    );

    let count: usize = txn
        .query_row(
            &sql,
            named_params! {
                ":invalid_link_hash": invalid_link_hash,
                ":create_link": DhtOpType::RegisterAddLink,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 1
}

// Now we're trying to remove an invalid link
fn expected_invalid_remove_link(txn: &Transaction, invalid_remove_hash: &ActionHash) -> bool {
    let sql = format!(
        "
        {}
        (type = :delete_link AND action_hash = :invalid_remove_hash
            AND validation_status = :rejected)
    ",
        SELECT
    );

    let count: usize = txn
        .query_row(
            &sql,
            named_params! {
                ":invalid_remove_hash": invalid_remove_hash,
                ":delete_link": DhtOpType::RegisterRemoveLink,
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
            "SELECT EXISTS(SELECT 1 FROM DhtOP WHERE when_integrated IS NULL)",
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
        let hash: ActionHash = row.get("hash")?;
        let action: SignedAction = from_blob(row.get("blob")?)?;
        Ok(DhtOpLite::from_type(op_type, hash, &action.0)?)
    })
    .unwrap()
    .collect::<StateQueryResult<Vec<DhtOpLite>>>()
    .unwrap()
}

fn num_valid(txn: &Transaction) -> usize {
    txn
    .query_row("SELECT COUNT(hash) FROM DhtOP WHERE when_integrated IS NOT NULL AND validation_status = :status",
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
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();

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
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

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
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

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
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

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
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

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
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

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
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

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
    let triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
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
    let triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
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
    let triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger(&"call_zome_directly");
    output
}
