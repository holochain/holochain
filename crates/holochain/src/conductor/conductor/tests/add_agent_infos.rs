use crate::{
    retry_until_timeout,
    sweettest::{SweetConductor, SweetDnaFile},
};
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn add_agent_infos_to_peer_store() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Crd])
        .await
        .0;
    // No apps nor agents created, should be empty.
    let agent_infos = conductor.get_agent_infos(None).await.unwrap();
    assert_eq!(agent_infos, vec![]);

    let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();

    let dna_id = app.cells()[0].dna_id().clone();
    let cell_peer_store = conductor
        .holochain_p2p
        .peer_store(dna_id.dna_hash().clone())
        .await
        .unwrap();
    // Await agent to be added to peer store.
    retry_until_timeout!({
        if cell_peer_store
            .get(dna_id.agent_pubkey().to_k2_agent())
            .await
            .unwrap()
            .is_some()
        {
            break;
        }
    });

    // Agent has been added to peer store.
    let expected_agent_infos = cell_peer_store.get_all().await.unwrap();
    let agent_infos = conductor.get_agent_infos(None).await.unwrap();
    assert_eq!(agent_infos, expected_agent_infos);

    // If dna id is passed into call, only the cell's agent info should be returned.
    let dna_id = app.cells()[0].dna_id().clone();
    let expected_agent_info = cell_peer_store
        .get(dna_id.agent_pubkey().to_k2_agent())
        .await
        .unwrap()
        .unwrap();
    let agent_infos = conductor
        .get_agent_infos(Some(vec![dna_id.dna_hash().clone()]))
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
