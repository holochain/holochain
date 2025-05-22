use crate::sweettest::*;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppRequest, AppResponse};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use kitsune2_api::AgentInfoSigned;
use kitsune2_core::Ed25519Verifier;

// in these tests we set up a mix of apps and including clone cells so we can test
// different varieties of combinations in the app_agent_info case, and we use the same setup in the admin_agent_info
// for parity.
async fn setup_tests() -> (
    DnaHash,
    DnaHash,
    DnaHash,
    String,
    String,
    ClonedCell,
    SweetConductor,
) {
    // Create three different DNAs
    let dna1 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna2 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna3 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Install two different apps on a conductor: app1 (dna1, dna2) and app2 (dna3)
    let config = SweetConductorConfig::standard();
    let mut conductor =
        SweetConductor::from_config_rendezvous(config, SweetLocalRendezvous::new().await).await;

    let app1_id: InstalledAppId = "app1".into();
    let app2_id: InstalledAppId = "app2".into();

    let installed_app1_id = conductor
        .setup_app(&app1_id, &[dna1.0.clone(), dna2.0.clone()])
        .await
        .unwrap()
        .installed_app_id()
        .clone();

    // Install app2 on conductors 1 and 2
    let installed_app2_id = conductor
        .setup_app(&app2_id, &[dna3.0.clone()])
        .await
        .unwrap()
        .installed_app_id()
        .clone();

    // Create a disabled clone cell for app1 on conductor[0]
    let clone_cell = conductor
        .create_clone_cell(
            &installed_app1_id,
            CreateCloneCellPayload {
                role_name: dna1.0.dna_hash().to_string(),
                modifiers: DnaModifiersOpt::none().with_network_seed("test_seed".to_string()),
                membrane_proof: None,
                name: Some("disabled_clone".into()),
            },
        )
        .await
        .unwrap();

    (
        dna1.0.dna_hash().clone(),
        dna2.0.dna_hash().clone(),
        dna3.0.dna_hash().clone(),
        installed_app1_id,
        installed_app2_id,
        clone_cell,
        conductor,
    )
}

fn make_agent(space: SpaceId) -> String {
    let mut builder = AgentBuilder::default();
    let local_agent: DynLocalAgent = Arc::new(Ed25519LocalAgent::default());
    builder.space = Some(space.clone());
    let info = builder.build(local_agent);
    info.encode().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_agent_info() {
    holochain_trace::test_run();

    let (
        dna1_hash,
        dna2_hash,
        dna3_hash,
        _installed_app1_id,
        _installed_app2_id,
        clone_cell,
        conductor,
    ) = setup_tests().await;

    // Get admin interface for conductor
    let (admin_sender, _admin_receiver) = conductor.admin_ws_client::<AdminResponse>().await;

    // Get all agent infos via admin interface
    let response = admin_sender
        .request(AdminRequest::AgentInfo { dna_hashes: None })
        .await
        .unwrap();
    let agent_infos = match response {
        AdminResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    assert_eq!(
        agent_infos.len(),
        4,
        "Should have agent_info for each dna on each conductor"
    );

    let mut seen_spaces = std::collections::HashSet::new();
    let mut seen_agents = std::collections::HashSet::new();
    for info in &agent_infos {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        seen_spaces.insert(decoded.space.clone());
        seen_agents.insert(decoded.agent.clone());
    }

    // Should have seen all DNA spaces
    assert_eq!(
        seen_spaces.len(),
        4,
        "The agent_infos should cover the 4 dnas"
    );
    assert!(seen_spaces.contains(&dna1_hash.to_k2_space()));
    assert!(seen_spaces.contains(&dna2_hash.to_k2_space()));
    assert!(seen_spaces.contains(&dna3_hash.to_k2_space()));
    assert!(seen_spaces.contains(&clone_cell.cell_id.dna_hash().to_k2_space()));

    assert_eq!(
        seen_agents.len(),
        2,
        "The agent_infos should cover the two agents (one for each app)"
    );

    let clone_cell_dna = clone_cell.cell_id.dna_hash();

    // Test getting agent info for the clone cell
    let response = admin_sender
        .request(AdminRequest::AgentInfo {
            dna_hashes: Some(vec![clone_cell_dna.clone()]),
        })
        .await
        .unwrap();
    let clone_agent_infos = match response {
        AdminResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have a single agent info
    assert_eq!(
        clone_agent_infos.len(),
        1,
        "Should have agent info for the clone cell"
    );

    // Verify all agent infos are for the clone cell's DNA
    for info in &clone_agent_infos {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert_eq!(
            decoded.space,
            clone_cell_dna.to_k2_space(),
            "Agent info should be for the clone cell's DNA"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn app_agent_info() {
    holochain_trace::test_run();

    let (
        dna1_hash,
        dna2_hash,
        dna3_hash,
        installed_app1_id,
        installed_app2_id,
        clone_cell,
        conductor,
    ) = setup_tests().await;

    // Test app1's agent info
    let (app1_sender, _app1_receiver) = conductor
        .app_ws_client::<AppResponse>(installed_app1_id.to_string())
        .await;

    // Test getting agent info for all cells in app1
    let response = app1_sender
        .request(AppRequest::AgentInfo { dna_hashes: None })
        .await
        .unwrap();
    let agent_infos = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have agent info for each DNA in app1
    assert_eq!(agent_infos.len(), 3);

    // Verify agent info content
    for info in &agent_infos {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert!(
            decoded.space == dna1_hash.to_k2_space()
                || decoded.space == dna2_hash.to_k2_space()
                || decoded.space == clone_cell.cell_id.dna_hash().to_k2_space(),
            "Agent info space should be one of app1's DNAs or the clone cell's DNA"
        );
    }

    // Test getting agent info for dna1 in app1
    let response = app1_sender
        .request(AppRequest::AgentInfo {
            dna_hashes: Some(vec![dna1_hash.clone()]),
        })
        .await
        .unwrap();
    let agent_infos_dna1 = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };
    // Should have just 1 agent info for the dna
    assert_eq!(agent_infos_dna1.len(), 1);

    for info in &agent_infos_dna1 {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert_eq!(
            decoded.space,
            dna1_hash.to_k2_space(),
            "Agent info should be for dna1"
        );
    }

    // Test app2's agent info
    let (app2_sender, _app2_receiver) = conductor
        .app_ws_client::<AppResponse>(installed_app2_id.to_string())
        .await;

    // Test getting agent info for all cells in app2
    let response = app2_sender
        .request(AppRequest::AgentInfo { dna_hashes: None })
        .await
        .unwrap();
    let agent_infos_app2 = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have agent info the dna in app2
    assert_eq!(agent_infos_app2.len(), 1);

    // Verify all agent infos are for dna3
    for info in &agent_infos_app2 {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert_eq!(
            decoded.space,
            dna3_hash.to_k2_space(),
            "Agent info should be for dna3"
        );
    }

    // Test getting agent info for non-existent DNA in app1
    let non_existent_dna = DnaHash::from_raw_32(vec![0; 32]);
    let response = app1_sender
        .request(AppRequest::AgentInfo {
            dna_hashes: Some(vec![non_existent_dna]),
        })
        .await
        .unwrap();
    let agent_infos_none = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have no agent infos for non-existent DNA
    assert_eq!(agent_infos_none.len(), 0);

    // Test getting agent info for dna3 (from app2) when querying from app1
    // This should return no results even though dna3 exists on the conductor in a different app
    let response = app1_sender
        .request(AppRequest::AgentInfo {
            dna_hashes: Some(vec![dna3_hash.clone()]),
        })
        .await
        .unwrap();
    let agent_infos_dna3_from_app1 = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have no agent infos for dna3 when querying from app1
    assert_eq!(agent_infos_dna3_from_app1.len(), 0,
        "Querying for dna3 from app1 should return no results, even though dna3 exists on conductor");

    // test when adding an new "external" agent_info
    let other_agent = make_agent(dna1_hash.to_k2_space());
    let (admin_sender, _admin_receiver) = conductor.admin_ws_client::<AdminResponse>().await;
    let _: AdminResponse = admin_sender
        .request(AdminRequest::AddAgentInfo {
            agent_infos: vec![other_agent],
        })
        .await
        .unwrap();

    // get the agent infos for app1 again.
    let response = app1_sender
        .request(AppRequest::AgentInfo {
            dna_hashes: Some(vec![dna1_hash.clone()]),
        })
        .await
        .unwrap();
    let agent_infos_dna1_from_app1 = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    assert_eq!(agent_infos_dna1_from_app1.len(), 2);
}
