use std::time::Duration;

use rusqlite::named_params;

use holo_hash::AnyDhtHash;
use holochain::sweettest::*;
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
    holochain_trace::test_run();
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let zomes = vec![TestWasm::Create];
    let mut conductor = SweetConductor::local_rendezvous().await;
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(zomes).await;
    let ((alice,), (bob,)) = conductor
        .setup_apps("app", 2, [&dna])
        .await
        .unwrap()
        .into_tuples();

    let handle = conductor.raw_handle();

    let _: ActionHash = conductor
        .call(&alice.zome(TestWasm::Create), "create_entry", ())
        .await;

    let record: Option<Record> = conductor
        .call(&alice.zome(TestWasm::Create), "get_entry", ())
        .await;

    let entry_hash = record.unwrap().action().entry_hash().cloned().unwrap();

    // publish these commits
    let triggers = handle.get_cell_triggers(alice.cell_id()).await.unwrap();
    triggers.integrate_dht_ops.trigger(&"authored_test");

    // Alice commits the entry
    alice
        .authored_db()
        .read_async({
            let basis: AnyDhtHash = entry_hash.clone().into();
            let alice_pk = alice.cell_id().agent_pubkey().clone();

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
        bob.dht_db(),
        expected_count,
        num_attempts,
        delay_per_attempt,
    )
    .await
    .unwrap();

    bob
        .authored_db()
        .read_async({
            let basis: AnyDhtHash = entry_hash.clone().into();
            let bob_pk = bob.cell_id().agent_pubkey().clone();

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

    bob.dht_db()
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
    let _: ActionHash = conductor
        .call(&bob.zome(TestWasm::Create), "create_entry", ())
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(bob.cell_id()).await.unwrap();
    triggers.publish_dht_ops.trigger(&"");

    bob
        .authored_db()
        .read_async({
            let basis: AnyDhtHash = entry_hash.clone().into();
            let bob_pk = bob.cell_id().agent_pubkey().clone();

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
}
