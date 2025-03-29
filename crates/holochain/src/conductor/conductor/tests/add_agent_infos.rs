use crate::sweettest::{SweetConductor, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky"]
async fn add_agent_infos_to_peer_store() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Crd])
        .await
        .0;
    // No apps nor agents created, should be empty.
    let agent_infos = conductor.get_agent_infos(None).await.unwrap();
    assert_eq!(agent_infos, vec![]);

    let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();
    // App with agent has been created, agent should be included.
    let cell_id = app.cells()[0].cell_id().clone();
    let cell_peer_store = conductor
        .holochain_p2p
        .peer_store(cell_id.dna_hash().clone())
        .await
        .unwrap();
    let expected_agent_infos = cell_peer_store.get_all().await.unwrap();
    let agent_infos = conductor.get_agent_infos(None).await.unwrap();
    assert_eq!(agent_infos, expected_agent_infos);

    // If cell id is passed into call, only the cell's agent info should be returned.
    let cell_id = app.cells()[0].cell_id().clone();
    let expected_agent_info = cell_peer_store
        .get(cell_id.agent_pubkey().to_k2_agent())
        .await
        .unwrap()
        .unwrap();
    let agent_infos = conductor
        .get_agent_infos(Some(cell_id.clone()))
        .await
        .unwrap();
    assert_eq!(agent_infos, vec![expected_agent_info.clone()]);

    drop(conductor);
    // Add agent info from first app installation to a new conductor's peer store.
    let new_agent_info = expected_agent_info.clone().encode().unwrap();
    let conductor = SweetConductor::from_standard_config().await;
    conductor
        .add_agent_infos(vec![new_agent_info.clone()])
        .await
        .unwrap();
    let agent_infos = conductor.get_agent_infos(None).await.unwrap();
    assert_eq!(agent_infos, vec![expected_agent_info]);
}
