use crate::sweettest::*;
use holochain_types::app::CreateCloneCellPayload;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{AppRoleId, get_clone_id};

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

// #[tokio::test(flavor = "multi_thread")]
// async fn create_clone_cell_creates_a_callable_cell() {
//     let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
//         .await
//         .unwrap();
//     let role_id: AppRoleId = "dna_1".to_string();
//     let mut conductor = SweetConductor::from_standard_config().await;
//     let alice = SweetAgents::one(conductor.keystore()).await;
//     let app = conductor
//         .setup_app_for_agent("app", alice.clone(), [DnaWithRole {dna, role_id}])
//         .await
//         .unwrap();

//     let clone_id = conductor
//         .clone()
//         .create_clone_cell(CreateCloneCellPayload {
//             app_id: app.installed_app_id().clone(),
//             role_id: role_id.clone(),
//             network_seed: Some("seed".to_string()),
//             properties: None,
//             membrane_proof: None,
//             name: None,
//             origin_time: None,
//         })
//         .await
//         .unwrap();
//     conductor.call_zome(ZomeCall {cell_id: ,cap_secret: None, payload: (), provenance: alice.clone(), fn_name: "".to_string(), zome_name: "".to_string()});
// }

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
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    assert_eq!(installed_clone_cell.into_role_id(), get_clone_id(&role_id, 0)); // clone index starts at 0
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
    assert_eq!(installed_clone_cell_0.into_role_id(), get_clone_id(&role_id, 0)); // clone index starts at 0

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
    assert_eq!(installed_clone_cell_1.into_role_id(), get_clone_id(&role_id, 1));
}
