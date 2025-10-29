use hdk::prelude::Record;
use holo_hash::ActionHash;
use holochain::sweettest::{
    await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
};
use holochain_wasm_test_utils::TestWasm;

/// For a test that checks that zero arc nodes block warranted agents, see [`super::warrants::zero_arc`].

#[tokio::test(flavor = "multi_thread")]
async fn publish_does_not_go_to_blocked_peers() {
    holochain_trace::test_run();
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let config = SweetConductorConfig::standard()
        .tune_conductor(|c| {
            c.min_publish_interval = Some(std::time::Duration::from_secs(2));
            c.publish_trigger_interval = Some(std::time::Duration::from_secs(3));
            c.sys_validation_retry_delay = Some(std::time::Duration::from_secs(1));
        })
        .tune_network_config(|nc| {
            // Publish should be the only sync method in this test.
            nc.disable_gossip = true;
        });
    let mut conductors = SweetConductorBatch::from_config(2, config).await;
    let apps = conductors.setup_app("create", [&dna_file]).await.unwrap();
    let ((alice_cell,), (bob_cell,)) = apps.into_tuples();
    let alice_conductor = conductors.get(0).unwrap();
    let bob_conductor = conductors.get(1).unwrap();
    let alice = alice_cell.zome(TestWasm::Create);
    let bob = bob_cell.zome(TestWasm::Create);

    let bob_pubkey = bob_cell.cell_id().agent_pubkey();

    // Both declare full arc, so that all actions are published to each other.
    alice_conductor
        .declare_full_storage_arcs(alice_cell.dna_hash())
        .await;
    bob_conductor
        .declare_full_storage_arcs(alice_cell.dna_hash())
        .await;

    // Await initial sync between Alice and Bob.
    await_consistency(30, [&alice_cell, &bob_cell])
        .await
        .unwrap();

    let action0: ActionHash = alice_conductor.call(&alice, "create_entry", ()).await;

    await_consistency(30, [&alice_cell, &bob_cell])
        .await
        .unwrap();

    // Before bob is blocked he can get posts just fine. This is a local get and gossip is disabled,
    // so all actions must have come in through publish.
    let bob_get0: Option<Record> = bob_conductor.call(&bob, "get_post", action0.clone()).await;
    assert!(bob_get0.is_some());

    // Bob gets blocked by Alice.
    let _block: () = alice_conductor
        .call(&alice, "block_agent", bob_pubkey)
        .await;

    let action1: ActionHash = alice_conductor.call(&alice, "create_entry", ()).await;

    // Consistency should not be reached, entry should not be published to Bob.
    await_consistency(10, [&alice_cell, &bob_cell])
        .await
        .unwrap_err();

    // Confirm that entry has not made it to Bob.
    let bob_get1: Option<Record> = bob_conductor.call(&bob, "get_post", action1.clone()).await;
    assert!(bob_get1.is_none());
}
