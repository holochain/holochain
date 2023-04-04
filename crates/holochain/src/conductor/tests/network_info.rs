use holochain_types::prelude::{InstalledAppId, NetworkInfoRequestPayload};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::RoleName;

use crate::sweettest::{SweetConductorBatch, SweetDnaFile};

#[tokio::test(flavor = "multi_thread")]
async fn network_info() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let number_of_peers = 3;
    let mut conductors = SweetConductorBatch::from_standard_config(number_of_peers).await;
    // let mut conductor = SweetConductor::from_standard_config().await;
    // let alice = SweetAgents::one(conductors[0].keystore()).await;
    // let bob = SweetAgents::one(conductors[1].keystore()).await;
    // let carol = SweetAgents::one(conductors[2].keystore()).await;
    let app_id: InstalledAppId = "app".into();
    // let role_name: RoleName = "role".into();
    // let ((alice, _), (bob, _), (carol, _)) = conductors
    let app_batch = conductors.setup_app(&app_id, &[dna.clone()]).await.unwrap();
    let apps = app_batch.into_inner();
    let alice_app = &apps[0];
    // let bob_app = &apps[1];
    conductors.exchange_peer_info().await;
    // .into_tuples();
    // .setup_app_for_agent(
    //     &app_id,
    //     alice.clone(),
    //     [&(role_name.clone(), dna.clone())],
    // )
    // .await
    // .unwrap();

    let payload = NetworkInfoRequestPayload {
        agent_pub_key: alice_app.agent().clone(),
        dnas: vec![dna.dna_hash().clone()],
        last_time_queried: None,
    };
    let network_info = conductors[0].network_info(&payload).await.unwrap();
    println!("a {:?}", network_info);
    assert_eq!(network_info[0].number_of_peers, 3);
    assert_eq!(network_info[0].arc_size, 1.0);
    assert_eq!(network_info[0].total_peers, vec![3.0]);
    assert_eq!(network_info[0].open_peer_connections, 0);
}
