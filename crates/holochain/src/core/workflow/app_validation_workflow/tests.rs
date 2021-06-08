use crate::conductor::ConductorHandle;
use crate::core::ribosome::ZomeCallInvocation;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::new_invocation;
use crate::test_utils::new_zome_call;
use crate::test_utils::setup_app;
use crate::test_utils::wait_for_integration;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::prelude::fresh_reader_test;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::StateQueryResult;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

use holochain_zome_types::Entry;
use holochain_zome_types::ValidationStatus;
use rusqlite::named_params;
use rusqlite::Transaction;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "(david.b) - not working locally"]
async fn app_validation_workflow_test() {
    observability::test_run_open().ok();

    let dna_file = DnaFile::new(
        DnaDef {
            name: "app_validation_workflow_test".to_string(),
            uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![
                TestWasm::Validate.into(),
                TestWasm::ValidateLink.into(),
                TestWasm::Create.into(),
            ]
            .into(),
        },
        vec![
            TestWasm::Validate.into(),
            TestWasm::ValidateLink.into(),
            TestWasm::Create.into(),
        ],
    )
    .await
    .unwrap();

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    let bob_agent_id = fake_agent_pubkey_2();
    let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
    let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(
            "test_app",
            vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        )],
        vec![dna_file.clone()],
    )
    .await;

    let expected_count = run_test(
        alice_cell_id.clone(),
        bob_cell_id.clone(),
        handle.clone(),
        &dna_file,
    )
    .await;
    run_test_entry_def_id(
        alice_cell_id,
        bob_cell_id,
        handle.clone(),
        &dna_file,
        expected_count,
    )
    .await;

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap().unwrap();
}

const SELECT: &'static str = "SELECT count(hash) FROM DhtOp WHERE";

// These are the expected invalid ops
fn expected_invalid_entry(
    txn: &Transaction,
    invalid_header_hash: &HeaderHash,
    invalid_entry_hash: &AnyDhtHash,
) -> bool {
    let sql = format!(
        "
        {}
        (
            (type = :store_entry AND header_hash = :invalid_header_hash 
                AND basis_hash = :invalid_entry_hash AND validation_status = :rejected)
            OR
            (type = :store_element AND header_hash = :invalid_header_hash 
                AND validation_status = :rejected)
        )
    ",
        SELECT
    );

    let count: usize = txn
        .query_row(
            &sql,
            named_params! {
                ":invalid_header_hash": invalid_header_hash,
                ":invalid_entry_hash": invalid_entry_hash,
                ":store_entry": DhtOpType::StoreEntry,
                ":store_element": DhtOpType::StoreElement,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 2
}

// Now we expect an invalid link
fn expected_invalid_link(txn: &Transaction, invalid_link_hash: &HeaderHash) -> bool {
    let sql = format!(
        "
        {}
        (
            (type = :create_link AND header_hash = :invalid_link_hash 
                AND validation_status = :rejected)
            OR
            (type = :store_element AND header_hash = :invalid_link_hash 
                AND validation_status = :rejected)
        )
    ",
        SELECT
    );

    let count: usize = txn
        .query_row(
            &sql,
            named_params! {
                ":invalid_link_hash": invalid_link_hash,
                ":create_link": DhtOpType::RegisterAddLink,
                ":store_element": DhtOpType::StoreElement,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 2
}

// Now we're trying to remove an invalid link
fn expected_invalid_remove_link(txn: &Transaction, invalid_remove_hash: &HeaderHash) -> bool {
    let sql = format!(
        "
        {}
        (
            (type = :delete_link AND header_hash = :invalid_remove_hash 
                AND validation_status = :rejected)
            OR
            (type = :store_element AND header_hash = :invalid_remove_hash 
                AND validation_status = :rejected)
        )
    ",
        SELECT
    );

    let count: usize = txn
        .query_row(
            &sql,
            named_params! {
                ":invalid_remove_hash": invalid_remove_hash,
                ":delete_link": DhtOpType::RegisterRemoveLink,
                ":store_element": DhtOpType::StoreElement,
                ":rejected": ValidationStatus::Rejected,
            },
            |row| row.get(0),
        )
        .unwrap();
    count == 2
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

fn show_limbo(txn: &Transaction) -> Vec<DhtOpLight> {
    txn.prepare("SELECT blob FROM DhtOp WHERE when_integrated IS NULL")
        .unwrap()
        .query_and_then([], |row| from_blob(row.get("blob")?))
        .unwrap()
        .collect::<StateQueryResult<Vec<DhtOpLight>>>()
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
    handle: ConductorHandle,
    dna_file: &DnaFile,
) -> usize {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let invocation =
        new_zome_call(&bob_cell_id, "always_validates", (), TestWasm::Validate).unwrap();
    handle.call_zome(invocation).await.unwrap().unwrap();

    // Integration should have 3 ops in it
    // Plus another 16 for genesis + init
    // Plus 2 for Cap Grant
    let expected_count = 3 + 16 + 2;
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(&alice_env, expected_count, num_attempts, delay_per_attempt).await;
    holochain_state::prelude::dump_tmp(&alice_env);

    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert_eq!(num_valid(&txn), expected_count);
    });

    let (invalid_header_hash, invalid_entry_hash) =
        commit_invalid(&bob_cell_id, &handle, dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    // Integration should have 3 ops in it
    // StoreEntry should be invalid.
    // RegisterAgentActivity doesn't run app validation
    // So they will be valid.
    let expected_count = 3 + expected_count;
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(&alice_env, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_header_hash,
            &invalid_entry_hash
        ));
        assert_eq!(num_valid(&txn), expected_count - 2);
    });

    let invocation =
        new_zome_call(&bob_cell_id, "add_valid_link", (), TestWasm::ValidateLink).unwrap();
    handle.call_zome(invocation).await.unwrap().unwrap();

    // Integration should have 6 ops in it
    let expected_count = 6 + expected_count;
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(&alice_env, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_header_hash,
            &invalid_entry_hash
        ));
        assert_eq!(num_valid(&txn), expected_count - 2);
    });

    let invocation =
        new_invocation(&bob_cell_id, "add_invalid_link", (), TestWasm::ValidateLink).unwrap();
    let invalid_link_hash: HeaderHash =
        call_zome_directly(&bob_cell_id, &handle, dna_file, invocation)
            .await
            .decode()
            .unwrap();

    // Integration should have 9 ops in it
    let expected_count = 9 + expected_count;
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(&alice_env, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_header_hash,
            &invalid_entry_hash
        ));
        assert!(expected_invalid_link(&txn, &invalid_link_hash));
        assert_eq!(num_valid(&txn), expected_count - 4);
    });

    let invocation = new_invocation(
        &bob_cell_id,
        "remove_valid_link",
        (),
        TestWasm::ValidateLink,
    )
    .unwrap();
    call_zome_directly(&bob_cell_id, &handle, dna_file, invocation).await;

    // Integration should have 9 ops in it
    let expected_count = 9 + expected_count;
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(&alice_env, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_header_hash,
            &invalid_entry_hash
        ));
        assert!(expected_invalid_link(&txn, &invalid_link_hash));
        assert_eq!(num_valid(&txn), expected_count - 4);
    });

    let invocation = new_invocation(
        &bob_cell_id,
        "remove_invalid_link",
        (),
        TestWasm::ValidateLink,
    )
    .unwrap();
    let invalid_remove_hash: HeaderHash =
        call_zome_directly(&bob_cell_id, &handle, dna_file, invocation)
            .await
            .decode()
            .unwrap();

    // Integration should have 12 ops in it
    let expected_count = 12 + expected_count;
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(&alice_env, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_header_hash,
            &invalid_entry_hash
        ));
        assert!(expected_invalid_link(&txn, &invalid_link_hash));
        assert!(expected_invalid_remove_link(&txn, &invalid_remove_hash));
        // 6 invalid ops above plus 2 extra invalid ops that `remove_invalid_link` commits.
        assert_eq!(num_valid(&txn), expected_count - (6 + 2));
    });
    expected_count
}

/// 1. Commits an entry with validate_create_entry_<EntryDefId> callback
/// 2. The callback rejects the entry proving that it actually ran.
/// 3. Reject only Post with "Banana" as the String to show it doesn't
///    affect other entries.
async fn run_test_entry_def_id(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    handle: ConductorHandle,
    dna_file: &DnaFile,
    expected_count: usize,
) {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let (invalid_header_hash, invalid_entry_hash) =
        commit_invalid_post(&bob_cell_id, &handle, dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    // Integration should have 3 ops in it
    // StoreEntry and StoreElement should be invalid.
    let expected_count = 3 + expected_count;
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(&alice_env, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_header_hash,
            &invalid_entry_hash
        ));
        assert_eq!(num_valid(&txn), expected_count - 10);
    });
}

// Need to "hack holochain" because otherwise the invalid
// commit is caught by the call zome workflow
async fn commit_invalid(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (HeaderHash, EntryHash) {
    let entry = ThisWasmEntry::NeverValidates;
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    // 4
    let invalid_header_hash = call_data
        .commit_entry(entry.clone().try_into().unwrap(), INVALID_ID)
        .await;

    // Produce and publish these commits
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger();
    (invalid_header_hash, entry_hash)
}

// Need to "hack holochain" because otherwise the invalid
// commit is caught by the call zome workflow
async fn commit_invalid_post(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (HeaderHash, EntryHash) {
    // Bananas are not allowed
    let entry = Post("Banana".into());
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    // Create call data for the 3rd zome Create
    let call_data = HostFnCaller::create_for_zome(bob_cell_id, handle, dna_file, 2).await;
    // 9
    let invalid_header_hash = call_data
        .commit_entry(entry.clone().try_into().unwrap(), POST_ID)
        .await;

    // Produce and publish these commits
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger();
    (invalid_header_hash, entry_hash)
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
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger();
    output
}
