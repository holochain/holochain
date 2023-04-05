use holo_hash::ActionHash;
use holochain_types::prelude::{InstalledAppId, NetworkInfoRequestPayload};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{ExternIO, Timestamp};

use crate::sweettest::{SweetConductorBatch, SweetDnaFile, SweetZome};

#[tokio::test(flavor = "multi_thread")]
async fn network_info() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let number_of_peers = 3;
    let mut conductors = SweetConductorBatch::from_standard_config(number_of_peers).await;
    let app_id: InstalledAppId = "app".into();
    let app_batch = conductors.setup_app(&app_id, &[dna.clone()]).await.unwrap();
    let apps = app_batch.into_inner();
    let alice_app = &apps[0];
    let bob_app = &apps[1];
    conductors.exchange_peer_info().await;

    conductors[0].persist();
    println!("{:?}", conductors[0].db_path());

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

    assert!(network_info[0].bytes_since_last_time_queried == 0);

    // create one entry
    let zome: SweetZome = SweetZome::new(
        alice_app.cells()[0].cell_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let a: ActionHash = conductors[0].call(&zome, "create_entry", ()).await;
    println!("a {:?}", a);

    // query for gossip rounds again
    let payload = NetworkInfoRequestPayload {
        agent_pub_key: bob_app.agent().clone(),
        dnas: vec![dna.dna_hash().clone()],
        last_time_queried: Some(last_time_queried),
    };
    let network_info = conductors[1].network_info(&payload).await.unwrap();
    println!("b {:?}", network_info);
    // assert_eq!(network_info[0].current_number_of_peers, 3);
    // assert_eq!(network_info[0].arc_size, 1.0);
    // assert_eq!(network_info[0].total_network_peers, 3);
    // assert_eq!(network_info[0].completed_rounds_since_last_time_queried, 0);
    // assert!(network_info[0].bytes_since_last_time_queried == 0);
}
