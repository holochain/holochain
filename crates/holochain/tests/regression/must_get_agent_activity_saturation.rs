use holo_hash::ActionHash;
use holochain::sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::Record;
use rand::{thread_rng, Rng};

// Intended to keep https://github.com/holochain/holochain/issues/3028 fixed.
// ensure that multiple `must_get_agent_activity` calls do not oversaturate the
// fetch pool and bring gossip to a halt
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn must_get_agent_activity_saturation() {
    use holochain::sweettest::await_consistency;

    holochain_trace::test_run();
    let mut rng = thread_rng();
    let (dna, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MustGetAgentActivity]).await;
    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;
    let apps = conductors
        .setup_app("", [&dna])
        .await
        .unwrap()
        .cells_flattened();
    let alice_cell = &apps[0];
    let bob_cell = &apps[1];

    let mut hash = ActionHash::from_raw_32(vec![0; 32]);
    for _ in 0..100 {
        let content: u32 = rng.gen();
        let record: Record = conductors[0]
            .call(
                &alice_cell.zome(TestWasm::MustGetAgentActivity.coordinator_zome_name()),
                "create_thing",
                content,
            )
            .await;
        hash = record.action_hashed().hash.clone();
    }

    // let conductors catch up
    await_consistency(60, [alice_cell, bob_cell]).await.unwrap();

    let record: Option<Record> = conductors[1]
        .call(
            &bob_cell.zome(TestWasm::MustGetAgentActivity.coordinator_zome_name()),
            "get_thing",
            hash,
        )
        .await;
    assert!(record.is_some());
}
