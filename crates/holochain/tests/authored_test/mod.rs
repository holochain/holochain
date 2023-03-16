use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::Duration;

use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_state::prelude::fresh_reader_test;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::Entry;

use holochain::test_utils::conductor_setup::ConductorTestData;
use holochain::test_utils::host_fn_caller::*;
use holochain::test_utils::wait_for_integration;
use holochain_zome_types::EntryDefLocation;
use holochain_zome_types::EntryVisibility;
use rusqlite::named_params;

/// - Alice commits an entry and it is in their authored store
/// - Bob doesn't have the entry in their authored store
/// - Bob does have the entry in their integrated store
/// - Bob commits the entry and it is now in their authored store
#[tokio::test(flavor = "multi_thread")]
async fn authored_test() {
    observability::test_run().ok();
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let zomes = vec![TestWasm::Create];
    let mut conductor_test = ConductorTestData::two_agents(zomes, true).await;
    let handle = conductor_test.handle();
    let alice_call_data = conductor_test.alice_call_data();
    let bob_call_data = conductor_test.bob_call_data().unwrap();

    let entry = Post("Hi there".into());
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    // 3
    let h = alice_call_data.get_api(TestWasm::Create);
    let zome_index = h.get_entry_type(TestWasm::Create, POST_INDEX).zome_index;
    h.commit_entry(
        entry.clone().try_into().unwrap(),
        EntryDefLocation::app(zome_index, POST_INDEX),
        EntryVisibility::Public,
    )
    .await;

    // publish these commits
    let triggers = handle
        .get_cell_triggers(&alice_call_data.cell_id)
        .await
        .unwrap();
    triggers.publish_dht_ops.trigger(&"");

    // Alice commits the entry
    fresh_reader_test(alice_call_data.authored_db.clone(), |txn| {
        let basis: AnyDhtHash = entry_hash.clone().into();
        let has_authored_entry: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp JOIN Action ON DhtOp.action_hash = Action.hash
                    WHERE basis_hash = :hash AND Action.author = :author)",
                named_params! {
                    ":hash": basis,
                    ":author": alice_call_data.cell_id.agent_pubkey(),
                },
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_authored_entry);
    });

    // Integration should have 3 ops in it.
    // Plus another 14 for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 3 + 14;

    wait_for_integration(
        &bob_call_data.dht_db,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    fresh_reader_test(bob_call_data.authored_db.clone(), |txn| {
        let basis: AnyDhtHash = entry_hash.clone().into();
        let has_authored_entry: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp JOIN Action ON DhtOp.action_hash = Action.hash
                    WHERE basis_hash = :hash AND Action.author = :author)",
                named_params! {
                    ":hash": basis,
                    ":author": bob_call_data.cell_id.agent_pubkey(),
                },
                |row| row.get(0),
            )
            .unwrap();
        // Bob Should not have the entry in their authored table
        assert!(!has_authored_entry);
    });
    fresh_reader_test(bob_call_data.dht_db.clone(), |txn| {
        let basis: AnyDhtHash = entry_hash.clone().into();
        let has_integrated_entry: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE basis_hash = :hash)",
                named_params! {
                    ":hash": basis,
                },
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_integrated_entry);
    });

    // Now bob commits the entry
    let h = bob_call_data.get_api(TestWasm::Create);
    let zome_index = h.get_entry_type(TestWasm::Create, POST_INDEX).zome_index;
    h.commit_entry(
        entry.clone().try_into().unwrap(),
        EntryDefLocation::app(zome_index, POST_INDEX),
        EntryVisibility::Public,
    )
    .await;

    // Produce and publish these commits
    let triggers = handle
        .get_cell_triggers(&bob_call_data.cell_id)
        .await
        .unwrap();
    triggers.publish_dht_ops.trigger(&"");

    fresh_reader_test(bob_call_data.authored_db.clone(), |txn| {
        let basis: AnyDhtHash = entry_hash.clone().into();
        let has_authored_entry: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp JOIN Action ON DhtOp.action_hash = Action.hash
                    WHERE basis_hash = :hash AND Action.author = :author)",
                named_params! {
                    ":hash": basis,
                    ":author": bob_call_data.cell_id.agent_pubkey(),
                },
                |row| row.get(0),
            )
            .unwrap();
        // Bob Should have the entry in their authored table because they committed it.
        assert!(has_authored_entry);
    });

    conductor_test.shutdown_conductor().await;
}
