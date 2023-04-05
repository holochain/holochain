use holochain_types::prelude::{InstalledAppId, NetworkInfoRequestPayload};
use holochain_wasm_test_utils::TestWasm;
// use holochain_zome_types::RoleName;

use crate::sweettest::{SweetConductorBatch, SweetDnaFile};

#[tokio::test(flavor = "multi_thread")]
async fn network_info() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let number_of_peers = 3;
    let mut conductors = SweetConductorBatch::from_standard_config(number_of_peers).await;
    let app_id: InstalledAppId = "app".into();
    let app_batch = conductors.setup_app(&app_id, &[dna.clone()]).await.unwrap();
    let apps = app_batch.into_inner();
    let alice_app = &apps[0];
    conductors.exchange_peer_info().await;

    let payload = NetworkInfoRequestPayload {
        agent_pub_key: alice_app.agent().clone(),
        dnas: vec![dna.dna_hash().clone()],
        last_time_queried: None,
    };
    let network_info = conductors[0].network_info(&payload).await.unwrap();

    assert_eq!(network_info[0].number_of_peers, 3);
    assert_eq!(network_info[0].arc_size, 1.0);
    assert_eq!(network_info[0].total_peers, 3);
    assert_eq!(network_info[0].completed_rounds_since_last_time_queried, 0);
    assert!(network_info[0].bytes_since_last_time_queried > 0);
}
