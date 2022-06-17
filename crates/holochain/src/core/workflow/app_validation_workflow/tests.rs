use crate::conductor::ConductorHandle;
use crate::core::ribosome::ZomeCallInvocation;
use crate::sweettest::SweetConductorBatch;
use crate::sweettest::SweetDnaFile;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::new_invocation;
use crate::test_utils::new_zome_call;
use crate::test_utils::wait_for_integration;
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
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
async fn app_validation_workflow_test() {
    observability::test_run().ok();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
        TestWasm::Validate,
        TestWasm::ValidateLink,
        TestWasm::Create,
    ])
    .await
    .unwrap();

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let apps = conductors
        .setup_app(&"test_app", &[dna_file.clone()])
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

fn show_limbo(txn: &Transaction) -> Vec<DhtOpLight> {
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
        Ok(DhtOpLight::from_type(op_type, hash, &action.0)?)
    })
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
    conductors: &SweetConductorBatch,
    dna_file: &DnaFile,
) -> usize {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let invocation =
        new_zome_call(&bob_cell_id, "always_validates", (), TestWasm::Validate).unwrap();
    conductors[1].call_zome(invocation).await.unwrap().unwrap();

    // Integration should have 3 ops in it
    // Plus another 16 for genesis + init
    // Plus 2 for Cap Grant
    let expected_count = 3 + 16 + 2;
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();

    fresh_reader_test(alice_db, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert_eq!(num_valid(&txn), expected_count);
    });

    let (invalid_action_hash, invalid_entry_hash) =
        commit_invalid(&bob_cell_id, &conductors[1].handle(), dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    // Integration should have 3 ops in it
    // StoreEntry should be invalid.
    // RegisterAgentActivity will be valid.
    let expected_count = 3 + expected_count;
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_db, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_action_hash,
            &invalid_entry_hash
        ));
        // Expect having one invalid op for the store entry.
        assert_eq!(num_valid(&txn), expected_count - 1);
    });

    let invocation =
        new_zome_call(&bob_cell_id, "add_valid_link", (), TestWasm::ValidateLink).unwrap();
    conductors[1].call_zome(invocation).await.unwrap().unwrap();

    // Integration should have 6 ops in it
    let expected_count = 6 + expected_count;
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_db, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_action_hash,
            &invalid_entry_hash
        ));
        // Expect having one invalid op for the store entry.
        assert_eq!(num_valid(&txn), expected_count - 1);
    });

    let invocation = new_invocation(
        &bob_cell_id,
        "add_invalid_link",
        (),
        TestWasm::ValidateLink.coordinator_zome(),
    )
    .unwrap();
    let invalid_link_hash: ActionHash =
        call_zome_directly(&bob_cell_id, &conductors[1].handle(), dna_file, invocation)
            .await
            .decode()
            .unwrap();

    // Integration should have 9 ops in it
    let expected_count = 9 + expected_count;
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_db, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_action_hash,
            &invalid_entry_hash
        ));
        assert!(expected_invalid_link(&txn, &invalid_link_hash));
        // Expect having two invalid ops for the two store entries.
        assert_eq!(num_valid(&txn), expected_count - 2);
    });

    let invocation = new_invocation(
        &bob_cell_id,
        "remove_valid_link",
        (),
        TestWasm::ValidateLink.coordinator_zome(),
    )
    .unwrap();
    call_zome_directly(&bob_cell_id, &conductors[1].handle(), dna_file, invocation).await;

    // Integration should have 9 ops in it
    let expected_count = 9 + expected_count;
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_db, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_action_hash,
            &invalid_entry_hash
        ));
        assert!(expected_invalid_link(&txn, &invalid_link_hash));
        // Expect having two invalid ops for the two store entries.
        assert_eq!(num_valid(&txn), expected_count - 2);
    });

    let invocation = new_invocation(
        &bob_cell_id,
        "remove_invalid_link",
        (),
        TestWasm::ValidateLink.coordinator_zome(),
    )
    .unwrap();
    let invalid_remove_hash: ActionHash =
        call_zome_directly(&bob_cell_id, &conductors[1].handle(), dna_file, invocation)
            .await
            .decode()
            .unwrap();

    // Integration should have 12 ops in it
    let expected_count = 12 + expected_count;
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_db, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        assert!(expected_invalid_entry(
            &txn,
            &invalid_action_hash,
            &invalid_entry_hash
        ));
        assert!(expected_invalid_link(&txn, &invalid_link_hash));
        assert!(expected_invalid_remove_link(&txn, &invalid_remove_hash));
        // 3 invalid ops above plus 1 extra invalid ops that `remove_invalid_link` commits.
        assert_eq!(num_valid(&txn), expected_count - (3 + 1));
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
        commit_invalid_post(&bob_cell_id, &conductors[1].handle(), dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    // Integration should have 3 ops in it
    // StoreEntry and StoreRecord should be invalid.
    let expected_count = 3 + expected_count;
    let alice_db = conductors[0].get_dht_db(&alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    fresh_reader_test(alice_db, |txn| {
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
    });
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
    // 4
    let invalid_action_hash = call_data
        .commit_entry(
            entry.clone().try_into().unwrap(),
            EntryDefIndex(0),
            EntryVisibility::Public,
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(&bob_cell_id).unwrap();
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
    let entry_index = call_data.get_entry_type(TestWasm::Create, POST_INDEX);
    // 9
    let invalid_action_hash = call_data
        .commit_entry(
            entry.clone().try_into().unwrap(),
            entry_index,
            EntryVisibility::Public,
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(&bob_cell_id).unwrap();
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
    let triggers = handle.get_cell_triggers(&bob_cell_id).unwrap();
    triggers.publish_dht_ops.trigger(&"call_zome_directly");
    output
}
