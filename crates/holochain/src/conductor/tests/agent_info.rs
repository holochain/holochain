use crate::sweettest::*;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppRequest, AppResponse};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use kitsune2_api::AgentInfoSigned;
use kitsune2_core::Ed25519Verifier;

// in these tests we set up a mix of apps on different conductors including clone cells so we can test
// different varieties of combinations in the app_agent_info case, and we use the same setup in the admin_agent_info
// for parity.
async fn setup_tests() -> (
    DnaHash,
    DnaHash,
    DnaHash,
    String,
    String,
    ClonedCell,
    SweetConductorBatch,
) {
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
    //    conductor0
    //        .disable_clone_cell(
    //            &installed_app1_id,
    //            &DisableCloneCellPayload {
    //                clone_cell_id: CloneCellId::CloneId(clone_cell.clone_id.clone()),
    //            },
    //        )
    //        .await
    //        .unwrap();

    // Create batch and add conductors
    let conductor_batch = SweetConductorBatch::new(vec![conductor0, conductor1, conductor2]);

    // Exchange peer info between conductors
    conductor_batch.exchange_peer_info().await;

    (
        dna1.0.dna_hash().clone(),
        dna2.0.dna_hash().clone(),
        dna3.0.dna_hash().clone(),
        installed_app1_id,
        installed_app2_id,
        clone_cell,
        conductor_batch,
    )
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
        conductor_batch,
    ) = setup_tests().await;

    // Get admin interface for conductor[0]
    let (admin_sender_0, _admin_receiver) = conductor_batch
        .get(0)
        .unwrap()
        .admin_ws_client::<AdminResponse>()
        .await;

    // Get all agent infos via admin interface
    let response = admin_sender_0
        .request(AdminRequest::AgentInfo { cell_id: None })
        .await
        .unwrap();
    let agent_infos = match response {
        AdminResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };
    println!("{} Agent Infos returned", agent_infos.len());
    let mut sorted_infos: Vec<std::sync::Arc<AgentInfoSigned>> = Vec::new();
    let mut seen_spaces = std::collections::HashSet::new();
    let mut seen_agents = std::collections::HashSet::new();
    for info in &agent_infos {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        seen_spaces.insert(decoded.space.clone());
        seen_agents.insert(decoded.agent.clone());
        println!("{:?}", decoded);
        println!("{:?}", info);

        sorted_infos.push(decoded);
    }
    sorted_infos.sort_by(|a, b| a.agent.cmp(&b.agent));
    for info in sorted_infos {
        println!("  {:?}", info);
    }

    println!("{} Spaces seen:", seen_spaces.len());
    for space in seen_spaces {
        println!("  {:?}", space);
    }

    println!("{} Agents seen:", seen_agents.len());
    for agent in seen_agents {
        println!("  {:?}", agent);
    }

    // TODO: is this really what we want to test?
    // assert_eq!(
    //     agent_infos.len(),
    //     7,
    //     "Should have agent_info for each dna on each conductor"
    // );

    // Should have seen all DNA spaces
    // println!("dn1k2: {}", &dna1_hash.to_k2_space());
    // assert!(seen_spaces.contains(&dna1_hash.to_k2_space()));
    // println!("dn2k2: {}", &dna2_hash.to_k2_space());
    // assert!(seen_spaces.contains(&dna2_hash.to_k2_space()));
    // println!("dn3k2: {}", &dna3_hash.to_k2_space());
    // assert!(seen_spaces.contains(&dna3_hash.to_k2_space()));

    // Test getting agent info for the clone cell
    let response = admin_sender_0
        .request(AdminRequest::AgentInfo {
            cell_id: Some(clone_cell.cell_id.clone()),
        })
        .await
        .unwrap();
    let clone_agent_infos = match response {
        AdminResponse::AgentInfo(infos) => infos,
        _ => panic!("Expected AgentInfo response"),
    };

    // Should have agent info for each conductor that has the clone cell (1 peers)
    assert_eq!(
        clone_agent_infos.len(),
        1,
        "Should have agent info for the clone cell on both conductors that have it"
    );

    // Verify all agent infos are for the clone cell's DNA
    for info in &clone_agent_infos {
        let decoded = AgentInfoSigned::decode(&Ed25519Verifier, info.as_bytes()).unwrap();
        assert_eq!(
            decoded.space,
            clone_cell.cell_id.dna_hash().to_k2_space(),
            "Agent info should be for the clone cell's DNA"
        );
    }
}
