use holochain_conductor_api::{AppRequest, AppResponse};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use kitsune2_api::AgentInfoSigned;
use kitsune2_core::Ed25519Verifier;
use crate::sweettest::*;

#[tokio::test(flavor = "multi_thread")]
async fn app_agent_info() {
    holochain_trace::test_run();

    // Create three different DNAs
    let dna1 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna2 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna3 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Create conductors individually
    let config = SweetConductorConfig::standard();
    let mut conductor0 = SweetConductor::from_config(config.clone()).await;
    let mut conductor1 = SweetConductor::from_config(config.clone()).await;
    let mut conductor2 = SweetConductor::from_config(config).await;

    // Install different apps on different conductors:
    // - conductor[0]: app1 (dna1, dna2)
    // - conductor[1]: app1 (dna1, dna2) and app2 (dna3)
    // - conductor[2]: app2 (dna3)
    let app1_id: InstalledAppId = "app1".into();
    let app2_id: InstalledAppId = "app2".into();

    // Install app1 on conductors 0 and 1
    let installed_app1_id = conductor0
        .setup_app(&app1_id, &[dna1.0.clone(), dna2.0.clone()])
        .await
        .unwrap()
        .installed_app_id()
        .clone();
    conductor1
        .setup_app(&app1_id, &[dna1.0.clone(), dna2.0.clone()])
        .await
        .unwrap();

    // Install app2 on conductors 1 and 2
    let installed_app2_id = conductor1
        .setup_app(&app2_id, &[dna3.0.clone()])
        .await
        .unwrap()
        .installed_app_id()
        .clone();
    conductor2
        .setup_app(&app2_id, &[dna3.0.clone()])
        .await
        .unwrap();

    // Create a disabled clone cell for app1 on conductor[0]
    let clone_cell_r = conductor0
        .create_clone_cell(
            &installed_app1_id,
            CreateCloneCellPayload {
                role_name: dna1.0.dna_hash().to_string(),
                modifiers: DnaModifiersOpt::none().with_network_seed("test_seed".to_string()),
                membrane_proof: None,
                name: Some("disabled_clone".into()),
            },
        )
        .await;
    let clone_cell = clone_cell_r.unwrap();
    conductor0
        .disable_clone_cell(
            &installed_app1_id,
            &DisableCloneCellPayload {
                clone_cell_id: CloneCellId::CloneId(clone_cell.clone_id.clone()),
            },
        )
        .await
        .unwrap();

    // Create batch and add conductors
    let conductor_batch = SweetConductorBatch::new(vec![conductor0, conductor1, conductor2]);

    // Exchange peer info between conductors
    conductor_batch.exchange_peer_info().await;

    // Test app1's agent info from conductor[0]
    let (app1_sender, _app1_receiver) = conductor_batch
        .get(0)
        .unwrap()
        .app_ws_client::<AppResponse>(installed_app1_id.to_string())
        .await;

    // Test getting agent info for all cells in app1
    let response = app1_sender
        .request(AppRequest::AgentInfo { dna_hash: None })
        .await
        .unwrap();
    let agent_infos = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have agent info for each conductor that has app1 (2 peers)
    assert_eq!(agent_infos.len(), 2);

    // Verify agent info content
    for info in &agent_infos {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert!(
            decoded.space == dna1.0.dna_hash().to_k2_space() || 
            decoded.space == dna2.0.dna_hash().to_k2_space() ||
            decoded.space == clone_cell.cell_id.dna_hash().to_k2_space(),
            "Agent info space should be one of app1's DNAs or the clone cell's DNA"
        );
    }
    println!("\n\n-------- DNA TEST 1 --------\n\n");

    // Test getting agent info for dna1 in app1
    let response = app1_sender
        .request(AppRequest::AgentInfo {
            dna_hash: Some(dna1.0.dna_hash().clone()),
        })
        .await
        .unwrap();
    let agent_infos_dna1 = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };
    // Should have agent info for each conductor that has app1 (2 peers)
    assert_eq!(agent_infos_dna1.len(), 2);

    // Verify all agent infos are for dna1
    for info in &agent_infos_dna1 {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert_eq!(
            decoded.space,
            dna1.0.dna_hash().to_k2_space(),
            "Agent info should be for dna1"
        );
    }

    // Test app2's agent info from conductor[2]
    let (app2_sender, _app2_receiver) = conductor_batch
        .get(2)
        .unwrap()
        .app_ws_client::<AppResponse>(installed_app2_id.to_string())
        .await;

    // Test getting agent info for all cells in app2
    let response = app2_sender
        .request(AppRequest::AgentInfo { dna_hash: None })
        .await
        .unwrap();
    let agent_infos_app2 = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have agent info for each conductor that has app2 (2 peers)
    assert_eq!(agent_infos_app2.len(), 2);

    // Verify all agent infos are for dna3
    for info in &agent_infos_app2 {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert_eq!(
            decoded.space,
            dna3.0.dna_hash().to_k2_space(),
            "Agent info should be for dna3"
        );
    }

    // Test getting agent info for non-existent DNA in app1
    let non_existent_dna = DnaHash::from_raw_32(vec![0; 32]);
    let response = app1_sender
        .request(AppRequest::AgentInfo {
            dna_hash: Some(non_existent_dna),
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
    // This should return no results even though dna3 exists on conductor[1]
    let response = app1_sender
        .request(AppRequest::AgentInfo {
            dna_hash: Some(dna3.0.dna_hash().clone()),
        })
        .await
        .unwrap();
    let agent_infos_dna3_from_app1 = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have no agent infos for dna3 when querying from app1
    assert_eq!(agent_infos_dna3_from_app1.len(), 0, 
        "Querying for dna3 from app1 should return no results, even though dna3 exists on conductor[1]");
} 