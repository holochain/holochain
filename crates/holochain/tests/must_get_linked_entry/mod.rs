use holo_hash::ActionHash;
use holochain::sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::Record;
use rand::{thread_rng, Rng};
use std::time::Duration;
use holochain::test_utils::consistency_60s;
use holochain_types::prelude::Link;

#[tokio::test(flavor = "multi_thread")]
async fn must_get_linked_entry() {
    holochain_trace::test_run().unwrap();

    let (dna, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MustGetLinkedEntry]).await;
    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;

    let apps = conductors
        .setup_app("", &[dna])
        .await
        .unwrap()
        .cells_flattened();

    let alice_app = &apps[0];
    let bob_app = &apps[1];

    conductors
        .require_initial_gossip_activity_for_cell(bob_app, Duration::from_secs(90))
        .await;

    let record: Record = conductors[0].call(
        &alice_app.zome(TestWasm::MustGetLinkedEntry.coordinator_zome_name()),
        "create_linked",
        5,
    ).await;

    consistency_60s([alice_app, bob_app]).await;

    let record: Vec<Link> = conductors[0].call(
        &alice_app.zome(TestWasm::MustGetLinkedEntry.coordinator_zome_name()),
        "get_linked",
        (),
    ).await;

    assert_eq!(1, record.len());
}
