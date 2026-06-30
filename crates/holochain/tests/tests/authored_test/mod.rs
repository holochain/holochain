use holo_hash::{AgentPubKey, AnyLinkableHash};
use holochain::sweettest::*;
use holochain::test_utils::wait_for_integration;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::*;
use std::time::Duration;

/// Returns whether `cell`'s merged store holds a locally-validated op at
/// `basis` whose action was authored by `author`.
///
/// The authored data now lives in the merged per-DNA store keyed by the real
/// action author, so "authored by X" means "an op at this basis whose action
/// author == X". This preserves the intent of the old authored-DB existence
/// query without relying on the now-empty per-agent authored database.
async fn store_has_op_at_basis_authored_by(
    cell: &SweetCell,
    basis: &AnyLinkableHash,
    author: &AgentPubKey,
) -> bool {
    let store = cell.dht_store();
    let read = store.as_read();
    for op_hash in read.get_ops_at_basis(basis).await.unwrap() {
        if let Some(sah) = read.action_for_op(&op_hash).await.unwrap() {
            if sah.action().author() == author {
                return true;
            }
        }
    }
    false
}

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
    let mut conductor = SweetConductor::standard().await;
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
    let basis: AnyLinkableHash = entry_hash.clone().into();

    // publish these commits
    let triggers = handle.get_cell_triggers(alice.cell_id()).await.unwrap();
    triggers.integrate_dht_ops.trigger(&"authored_test");

    // Alice commits the entry, so the store has an op at that basis authored
    // by Alice.
    assert!(
        store_has_op_at_basis_authored_by(&alice, &basis, alice.agent_pubkey()).await,
        "alice should have an op at the entry basis authored by alice"
    );

    // Integration should have 3 ops in it.
    // Plus another 14 (2x7) for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 3 + 14;

    wait_for_integration(
        bob.dht_store(),
        expected_count as u64,
        num_attempts,
        delay_per_attempt,
    )
    .await;

    // Bob has not authored anything at this basis (Alice did), so there is no
    // op at the basis authored by Bob.
    assert!(
        !store_has_op_at_basis_authored_by(&bob, &basis, bob.agent_pubkey()).await,
        "bob should not have an op at the entry basis authored by bob"
    );

    // But Bob does hold Alice's integrated op at that basis.
    let ops_at_basis = bob
        .dht_store()
        .as_read()
        .get_ops_at_basis(&basis)
        .await
        .unwrap();
    assert!(
        !ops_at_basis.is_empty(),
        "bob should have an integrated op at the entry basis"
    );

    // Now bob commits the entry
    let _: ActionHash = conductor
        .call(&bob.zome(TestWasm::Create), "create_entry", ())
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(bob.cell_id()).await.unwrap();
    triggers.publish_dht_ops.trigger(&"");

    // Bob should now have an op at the entry basis authored by Bob, because
    // they committed it.
    assert!(
        store_has_op_at_basis_authored_by(&bob, &basis, bob.agent_pubkey()).await,
        "bob should have an op at the entry basis authored by bob after committing"
    );
}
