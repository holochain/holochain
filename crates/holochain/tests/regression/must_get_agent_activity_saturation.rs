use holo_hash::ActionHash;
use holochain::sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::Record;
use rand::{thread_rng, Rng};
use std::time::Duration;

// Intended to keep https://github.com/holochain/holochain/issues/3028 fixed.
// ensure that multiple `must_get_agent_activity` calls do not oversaturate the
// fetch pool and bring gossip to a halt
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
#[ignore = "temporarily while working on app validation refactor; to be reinstated along the refactor"]
async fn must_get_agent_activity_saturation() {
    holochain_trace::test_run().ok();
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
    let alice_app = &apps[0];
    let bob_app = &apps[1];

    let mut hash = ActionHash::from_raw_32(vec![0; 32]);
    for _ in 0..100 {
        let content: u32 = rng.gen();
        let record: Record = conductors[0]
            .call(
                &alice_app.zome(TestWasm::MustGetAgentActivity.coordinator_zome_name()),
                "create_thing",
                content,
            )
            .await;
        hash = record.action_hashed().hash.clone();
    }

    // let conductors catch up
    tokio::time::sleep(Duration::from_secs(60)).await;

    let record: Option<Record> = conductors[1]
        .call(
            &bob_app.zome(TestWasm::MustGetAgentActivity.coordinator_zome_name()),
            "get_thing",
            hash,
        )
        .await;
    assert!(matches!(record, Some(_)));
}
