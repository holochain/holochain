use holo_hash::ActionHash;
use holochain_types::prelude::{InstalledAppId, NetworkInfoRequestPayload};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::Timestamp;

use crate::sweettest::*;

#[tokio::test(flavor = "multi_thread")]
async fn network_info() {
    holochain_trace::test_run().ok();

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let number_of_peers = 3;
    let config = SweetConductorConfig::standard();
    let mut conductors = SweetConductorBatch::from_config(number_of_peers, config).await;
    let app_id: InstalledAppId = "app".into();
    let app_batch = conductors.setup_app(&app_id, &[dna.clone()]).await.unwrap();
    let cells = app_batch.cells_flattened();
    let apps = app_batch.into_inner();
    let alice_app = &apps[0];
    let bob_app = &apps[1];

    conductors.exchange_peer_info().await;

    // query since beginning of unix epoch
    let payload = NetworkInfoRequestPayload {
        agent_pub_key: alice_app.agent().clone(),
        dnas: vec![dna.dna_hash().clone()],
        last_time_queried: None,
    };
    let network_info = conductors[0].network_info(&payload).await.unwrap();

    assert_eq!(network_info[0].current_number_of_peers, 3);
    assert_eq!(network_info[0].arc_size, 1.0);
    assert_eq!(network_info[0].total_network_peers, 3);
    assert_eq!(network_info[0].completed_rounds_since_last_time_queried, 0);
    assert!(network_info[0].bytes_since_last_time_queried > 0);

    // query since previous query should return 0 received bytes
    let last_time_queried = Timestamp::now();
    let payload = NetworkInfoRequestPayload {
        agent_pub_key: alice_app.agent().clone(),
        dnas: vec![dna.dna_hash().clone()],
        last_time_queried: Some(last_time_queried),
    };
    let network_info = conductors[0].network_info(&payload).await.unwrap();

    assert_eq!(network_info[0].bytes_since_last_time_queried, 0);

    let cell = alice_app.cells()[0].clone();
    // alice creates one entry
    let zome = SweetZome::new(
        cell.cell_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let _: ActionHash = conductors[0].call(&zome, "create_entry", ()).await;

    await_consistency(10, &cells).await.unwrap();

    // wait_for_integration(
    //     &conductors[1].get_dht_db(dna.dna_hash()).unwrap(),
    //     28,
    //     100,
    //     std::time::Duration::from_millis(100),
    // )
    // .await;

    // query bob's DB for bytes since last time queried
    let payload = NetworkInfoRequestPayload {
        agent_pub_key: bob_app.agent().clone(),
        dnas: vec![dna.dna_hash().clone()],
        last_time_queried: Some(last_time_queried),
    };
    let network_info = conductors[1].network_info(&payload).await.unwrap();
    assert!(network_info[0].bytes_since_last_time_queried > 0);
}
