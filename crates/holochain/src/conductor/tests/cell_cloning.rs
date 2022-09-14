use crate::sweettest::*;
use holo_hash::ActionHash;
use holochain_types::app::CreateCloneCellPayload;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{AppRoleId, CloneId, DnaPhenotypeOpt};

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_without_phenotype_options_fails() {
    let conductor = SweetConductor::from_standard_config().await;
    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "".to_string(),
            role_id: "".to_string(),
            phenotype: DnaPhenotypeOpt {
                network_seed: None,
                properties: None,
                origin_time: None,
            },
            membrane_proof: None,
            name: None,
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_with_wrong_app_or_role_id_fails() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
        .await
        .unwrap();
    let role_id: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_id.clone(), dna)])
        .await
        .unwrap();

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "wrong_app_id".to_string(),
            role_id: role_id.clone(),
            phenotype: DnaPhenotypeOpt {
                network_seed: Some("seed".to_string()),
                properties: None,
                origin_time: None,
            },
            membrane_proof: None,
            name: None,
        })
        .await;
    assert!(result.is_err());

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: "wrong_role_id".to_string(),
            phenotype: DnaPhenotypeOpt {
                network_seed: Some("seed".to_string()),
                properties: None,
                origin_time: None,
            },
            membrane_proof: None,
            name: None,
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_run_twice_returns_correct_clone_indexes() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
        .await
        .unwrap();
    let role_id: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_id.clone(), dna)])
        .await
        .unwrap();

    let installed_clone_cell_0 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role_id.clone(),
            phenotype: DnaPhenotypeOpt {
                network_seed: Some("seed_1".to_string()),
                properties: None,
                origin_time: None,
            },
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(
        installed_clone_cell_0.into_role_id(),
        *CloneId::new(&role_id, 0).as_app_role_id()
    ); // clone index starts at 0

    let installed_clone_cell_1 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role_id.clone(),
            phenotype: DnaPhenotypeOpt {
                network_seed: Some("seed_2".to_string()),
                properties: None,
                origin_time: None,
            },
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(
        installed_clone_cell_1.into_role_id(),
        *CloneId::new(&role_id, 1).as_app_role_id()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_creates_callable_cell() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
        .await
        .unwrap();
    let role_id: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_id.clone(), dna.clone())])
        .await
        .unwrap();

    let installed_clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role_id.clone(),
            phenotype: DnaPhenotypeOpt {
                network_seed: Some("seed".to_string()),
                properties: None,
                origin_time: None,
            },
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();
    let zome = SweetZome::new(
        installed_clone_cell.as_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn app_info_includes_cloned_cells() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
        .await
        .unwrap();
    let role_id: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app_id = "app";
    conductor
        .setup_app_for_agent(app_id, alice.clone(), [&(role_id.clone(), dna)])
        .await
        .unwrap();
    let installed_clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app_id.to_string(),
            role_id: role_id.clone(),
            phenotype: DnaPhenotypeOpt {
                network_seed: Some("seed_1".to_string()),
                properties: None,
                origin_time: None,
            },
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();

    let app_info = conductor
        .get_app_info(&app_id.to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app_info.cell_data.len(), 2);
    assert!(app_info.cell_data.contains(&installed_clone_cell));
}
