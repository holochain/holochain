use crate::sweettest::*;
use holo_hash::ActionHash;
use holochain_types::{
    app::CreateCloneCellPayload,
    prelude::{CloneCellId, DeleteCloneCellPayload},
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{AppRoleId, CloneId};

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_without_network_seed_or_properties_fails() {
    let conductor = SweetConductor::from_standard_config().await;
    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "".to_string(),
            role_id: "".to_string(),
            network_seed: None,
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_with_wrong_app_id_fails() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
        .await
        .unwrap();
    let role_id: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_id.clone(), dna)])
        .await
        .unwrap();

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "wrong_app_id".to_string(),
            role_id: role_id.clone(),
            network_seed: Some("seed".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_with_wrong_role_id_fails() {
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
            app_id: app.installed_app_id().clone(),
            role_id: "wrong_role_id".to_string(),
            network_seed: Some("seed".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_returns_clone_id_with_correct_role_id() {
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

    let installed_clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role_id.clone(),
            network_seed: Some("seed".to_string()),
            properties: None,
            membrane_proof: None,
            name: Some("peter".to_string()),
            origin_time: None,
        })
        .await
        .unwrap();
    assert_eq!(
        installed_clone_cell.into_role_id(),
        CloneId::new(&role_id, 0).as_app_role_id()
    ); // clone index starts at 0
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
            network_seed: Some("seed_1".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    assert_eq!(
        installed_clone_cell_0.into_role_id(),
        CloneId::new(&role_id, 0).as_app_role_id()
    ); // clone index starts at 0

    let installed_clone_cell_1 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role_id.clone(),
            network_seed: Some("seed_2".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    assert_eq!(
        installed_clone_cell_1.into_role_id(),
        CloneId::new(&role_id, 1).as_app_role_id()
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
            network_seed: Some("seed".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
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
async fn delete_clone_cell_by_clone_id_marks_clone_cell_deleted() {
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
    let installed_clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role_id.clone(),
            network_seed: Some("seed_1".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    let result = conductor
        .clone()
        .destroy_clone_cell(DeleteCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.into_role_id()).unwrap(),
            ),
        })
        .await
        .unwrap();
    assert_eq!(result, true);
}
