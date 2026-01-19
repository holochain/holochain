use crate::sweettest::SweetLocalRendezvous;
use crate::{
    retry_until_timeout,
    sweettest::{SweetConductor, SweetConductorConfig, SweetDnaFile},
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

    let app = conductor
        .setup_app("", std::slice::from_ref(&dna_file))
        .await
        .unwrap();

    let cell_id = app.cells()[0].cell_id().clone();
    let cell_peer_store = conductor
        .holochain_p2p
        .peer_store(cell_id.dna_hash().clone())
        .await
        .unwrap();
    // Await agent to be added to peer store.
    retry_until_timeout!({
        if cell_peer_store
            .get(cell_id.agent_pubkey().to_k2_agent())
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

    // If cell id is passed into call, only the cell's agent info should be returned.
    let cell_id = app.cells()[0].cell_id().clone();
    let expected_agent_info = cell_peer_store
        .get(cell_id.agent_pubkey().to_k2_agent())
        .await
        .unwrap()
        .unwrap();
    let agent_infos = conductor
        .get_agent_infos(Some(vec![cell_id.dna_hash().clone()]))
        .await
        .unwrap();
    assert_eq!(agent_infos, vec![expected_agent_info.clone()]);

    drop(conductor);

    // Add agent info from first app installation to a new conductor's peer store.
    let mut conductor = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(false),
        SweetLocalRendezvous::new().await,
    )
    .await;

    // Install an app with the same DNA to create the space first
    let _app: crate::sweettest::SweetApp = conductor
        .setup_app("", std::slice::from_ref(&dna_file))
        .await
        .unwrap();

    // Wait for the new conductor's agent to be added to peer store
    let new_cell_id = _app.cells()[0].cell_id().clone();
    let cell_peer_store = conductor
        .holochain_p2p
        .peer_store(new_cell_id.dna_hash().clone())
        .await
        .unwrap();
    retry_until_timeout!({
        if cell_peer_store
            .get(new_cell_id.agent_pubkey().to_k2_agent())
            .await
            .unwrap()
            .is_some()
        {
            break;
        }
    });

    // Add the agent info from the first conductor
    conductor
        .add_agent_infos(vec![expected_agent_info.clone().encode().unwrap()])
        .await
        .unwrap();

    let agent_infos = conductor.get_agent_infos(None).await.unwrap();
    assert!(agent_infos.contains(&expected_agent_info));
}
