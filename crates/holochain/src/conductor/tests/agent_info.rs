use crate::{sweettest::*, test_utils::retry_fn_until_timeout};
use holochain_conductor_api::{AdminRequest, AdminResponse, AppRequest, AppResponse};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use kitsune2_api::{AgentInfoSigned, DynLocalAgent};
use kitsune2_core::{Ed25519LocalAgent, Ed25519Verifier};
use kitsune2_test_utils::agent::AgentBuilder;
use std::sync::Arc;

// in these tests we set up a mix of apps and including clone cells so we can test
// different varieties of combinations in the app_agent_info case, and we use the same setup in the admin_agent_info
// for parity.
async fn setup_tests() -> (
    DnaHash,
    DnaHash,
    DnaHash,
    DnaHash,
    String,
    String,
    String,
    ClonedCell,
    SweetConductorBatch,
) {
    // Create four different DNAs
    let dna1 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna2 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna3 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let dna4 = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    // Install two different apps on one conductor: app1 (dna1, dna2) and app2 (dna3)
    // Install another app on both conductors: app3 (dna4)
    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;

    let app1_id: InstalledAppId = "app1".into();
    let app2_id: InstalledAppId = "app2".into();
    let app3_id: InstalledAppId = "app3".into();

    // Install app1
    let installed_app1_id = conductors[0]
        .setup_app(&app1_id, &[dna1.0.clone(), dna2.0.clone()])
        .await
        .unwrap()
        .installed_app_id()
        .clone();

    // Install app2
    let installed_app2_id = conductors[0]
        .setup_app(&app2_id, std::slice::from_ref(&dna3.0))
        .await
        .unwrap()
        .installed_app_id()
        .clone();

    // Create a disabled clone cell for app1
    let clone_cell = conductors[0]
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

    // Install app3 on both conductors
    let _ = conductors[0]
        .setup_app(&app3_id, std::slice::from_ref(&dna4.0))
        .await
        .unwrap();

    let installed_app3_id = conductors[1]
        .setup_app(&app3_id, std::slice::from_ref(&dna4.0))
        .await
        .unwrap()
        .installed_app_id()
        .clone();

    // Wait until all peers are added to the peer stores of both conductors
    retry_fn_until_timeout(
        || async {
            futures::future::join_all(
                [
                    dna1.0.dna_hash(),
                    dna2.0.dna_hash(),
                    dna3.0.dna_hash(),
                    clone_cell.cell_id.dna_hash(),
                ]
                .map(|dna_hash| async {
                    conductors[0]
                        .holochain_p2p()
                        .peer_store(dna_hash.clone())
                        .await
                        .unwrap()
                        .get_all()
                        .await
                        .unwrap()
                        .len()
                }),
            )
            .await
            .iter()
            .all(|num_agents| *num_agents == 1)
                && conductors[0]
                    .holochain_p2p()
                    .peer_store(dna4.0.dna_hash().clone())
                    .await
                    .unwrap()
                    .get_all()
                    .await
                    .unwrap()
                    .len()
                    == 2
                && conductors[1]
                    .holochain_p2p()
                    .peer_store(dna4.0.dna_hash().clone())
                    .await
                    .unwrap()
                    .get_all()
                    .await
                    .unwrap()
                    .len()
                    == 2
        },
        None,
        None,
    )
    .await
    .unwrap();

    (
        dna1.0.dna_hash().clone(),
        dna2.0.dna_hash().clone(),
        dna3.0.dna_hash().clone(),
        dna4.0.dna_hash().clone(),
        installed_app1_id,
        installed_app2_id,
        installed_app3_id,
        clone_cell,
        conductors,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_agent_info() {
    holochain_trace::test_run();

    let (
        dna1_hash,
        dna2_hash,
        dna3_hash,
        dna4_hash,
        _installed_app1_id,
        _installed_app2_id,
        _installed_app3_id,
        clone_cell,
        conductors,
    ) = setup_tests().await;

    // Get admin interface for conductor 1
    let (admin_sender, _admin_receiver) = conductors[0].admin_ws_client::<AdminResponse>().await;

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
        6,
        "Should have agent_info for each dna on both conductors"
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
        5,
        "The agent_infos should cover the 4 dnas"
    );
    assert!(seen_spaces.contains(&dna1_hash.to_k2_space()));
    assert!(seen_spaces.contains(&dna2_hash.to_k2_space()));
    assert!(seen_spaces.contains(&dna3_hash.to_k2_space()));
    assert!(seen_spaces.contains(&clone_cell.cell_id.dna_hash().to_k2_space()));

    assert_eq!(
        seen_agents.len(),
        4,
        "The agent_infos should cover the two agents (one for each app and conductor)"
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

    // Test getting remote agent info for the DNA that is run by both conductors
    let response = admin_sender
        .request(AdminRequest::AgentInfo {
            dna_hashes: Some(vec![dna4_hash.clone()]),
        })
        .await
        .unwrap();
    let infos = match response {
        AdminResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };
    assert_eq!(
        infos.len(),
        2,
        "Should have agent info for agents in local and remote conductors"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn app_agent_info() {
    holochain_trace::test_run();

    let (
        dna1_hash,
        dna2_hash,
        dna3_hash,
        dna4_hash,
        installed_app1_id,
        installed_app2_id,
        installed_app3_id,
        clone_cell,
        conductors,
    ) = setup_tests().await;

    // Test app1's agent info
    let (app1_sender, _app1_receiver) = conductors[0]
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
    let (app2_sender, _app2_receiver) = conductors[0]
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
    let other_agent = AgentBuilder {
        space_id: Some(dna1_hash.to_k2_space()),
        ..Default::default()
    }
    .build(Arc::new(Ed25519LocalAgent::default()) as DynLocalAgent)
    .encode()
    .unwrap();

    let (admin_sender, _admin_receiver) = conductors[0].admin_ws_client::<AdminResponse>().await;
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

    // Test app3's agent info
    let (app3_sender, _app3_receiver) = conductors[0]
        .app_ws_client::<AppResponse>(installed_app3_id.to_string())
        .await;

    // Test getting remote agent info for the DNA that is run by both conductors
    let response = app3_sender
        .request(AppRequest::AgentInfo {
            dna_hashes: Some(vec![dna4_hash.clone()]),
        })
        .await
        .unwrap();
    let infos = match response {
        AppResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };
    assert_eq!(
        infos.len(),
        2,
        "Should have agent info for agents in local and remote conductors"
    );
}
