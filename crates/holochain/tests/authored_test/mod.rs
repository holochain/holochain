use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::Duration;

use rusqlite::named_params;

use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain::test_utils::conductor_setup::ConductorTestData;
use holochain::test_utils::host_fn_caller::*;
use holochain::test_utils::wait_for_integration;
use holochain_sqlite::error::DatabaseResult;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::*;

/// - Alice commits an entry and it is in their authored store
/// - Bob doesn't have the entry in their authored store
/// - Bob does have the entry in their integrated store
/// - Bob commits the entry and it is now in their authored store
#[tokio::test(flavor = "multi_thread")]
async fn authored_test() {
    holochain_trace::test_run().ok();
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
    triggers.integrate_dht_ops.trigger(&"authored_test");

    // Alice commits the entry
    alice_call_data
        .authored_db
        .read_async({
            let basis: AnyDhtHash = entry_hash.clone().into();
            let alice_pk = alice_call_data.cell_id.agent_pubkey().clone();

            move |txn| -> DatabaseResult<()> {
                let has_authored_entry: bool = txn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM DhtOp JOIN Action ON DhtOp.action_hash = Action.hash
                    WHERE basis_hash = :hash AND Action.author = :author)",
                    named_params! {
                    ":hash": basis,
                    ":author": alice_pk,
                },
                    |row| row.get(0),
                )?;

                assert!(has_authored_entry);

                Ok(())
            }
        })
        .await
        .unwrap();

    // Integration should have 3 ops in it.
    // Plus another 14 (2x7) for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 3 + 14;

    wait_for_integration(
        &bob_call_data.dht_db,
        expected_count,
        num_attempts,
        delay_per_attempt,
    )
    .await;

    bob_call_data
        .authored_db
        .read_async({
            let basis: AnyDhtHash = entry_hash.clone().into();
            let bob_pk = bob_call_data.cell_id.agent_pubkey().clone();

            move |txn| -> DatabaseResult<()> {
                let has_authored_entry: bool = txn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM DhtOp JOIN Action ON DhtOp.action_hash = Action.hash
                    WHERE basis_hash = :hash AND Action.author = :author)",
                    named_params! {
                    ":hash": basis,
                    ":author": bob_pk,
                },
                    |row| row.get(0),
                )?;
                // Bob Should not have the entry in their authored table
                assert!(!has_authored_entry);

                Ok(())
            }
        })
        .await
        .unwrap();

    bob_call_data
        .dht_db
        .read_async({
            let basis: AnyDhtHash = entry_hash.clone().into();

            move |txn| -> DatabaseResult<()> {
                let has_integrated_entry: bool = txn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE basis_hash = :hash)",
                    named_params! {
                        ":hash": basis,
                    },
                    |row| row.get(0),
                )?;

                assert!(has_integrated_entry);

                Ok(())
            }
        })
        .await
        .unwrap();

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

    bob_call_data
        .authored_db
        .read_async({
            let basis: AnyDhtHash = entry_hash.clone().into();
            let bob_pk = bob_call_data.cell_id.agent_pubkey().clone();

            move |txn| -> DatabaseResult<()> {
                let has_authored_entry: bool = txn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM DhtOp JOIN Action ON DhtOp.action_hash = Action.hash
                    WHERE basis_hash = :hash AND Action.author = :author)",
                    named_params! {
                    ":hash": basis,
                    ":author": bob_pk,
                },
                    |row| row.get(0),
                )?;

                // Bob Should have the entry in their authored table because they committed it.
                assert!(has_authored_entry);

                Ok(())
            }
        })
        .await
        .unwrap();

    conductor_test.shutdown_conductor().await;
}
