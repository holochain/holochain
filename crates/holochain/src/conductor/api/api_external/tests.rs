//! docs

use crate::sweettest::*;
use holochain_types::app::CreateCloneCellPayload;
use holochain_wasm_test_utils::TestWasm;

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
    let role_id = dna.dna_hash().to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    conductor
        .setup_app_for_agent("app", alice.clone(), [&dna])
        .await
        .unwrap();

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "wrong_app_id".to_string(),
            role_id: role_id.clone(),
            network_seed: Some("seed".to_string()),
            properties: None,
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
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna])
        .await
        .unwrap();

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: "wrong_role_id".to_string(),
            network_seed: Some("seed".to_string()),
            properties: None,
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
    let role_id = dna.dna_hash().to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna])
        .await
        .unwrap();

    let clone_id = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role_id.clone(),
            network_seed: Some("seed".to_string()),
            properties: None,
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    println!("clone_id {:?}", clone_id);
    assert_eq!(clone_id, format!("{}.{}", role_id, 0)); // clone cell's index starts at 0
}

// #[tokio::test(flavor = "multi_thread")]
// async fn create_clone_cell_run_twice_returns_correct_clone_indexes() {
//     let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
//         .await
//         .unwrap();
//     let role_id = dna.dna_hash().to_string();
//     let mut conductor = SweetConductor::from_standard_config().await;
//     let alice = SweetAgents::one(conductor.keystore()).await;
//     let app = conductor
//         .setup_app_for_agent("app", alice.clone(), [&dna])
//         .await
//         .unwrap();

//     let clone_id_0 = conductor
//         .clone()
//         .create_clone_cell(CreateCloneCellPayload {
//             app_id: app.installed_app_id().clone(),
//             role_id: role_id.clone(),
//             network_seed: Some("seed".to_string()),
//             properties: None,
//             name: None,
//             origin_time: None,
//         })
//         .await
//         .unwrap();
//     println!("clone_id_0 {}", clone_id_0);
//     assert_eq!(clone_id_0, format!("{}.{}", role_id, 0)); // clone cell's index starts at 0

//     let clone_id_1 = conductor
//         .clone()
//         .create_clone_cell(CreateCloneCellPayload {
//             app_id: app.installed_app_id().clone(),
//             role_id: role_id.clone(),
//             network_seed: Some("seed".to_string()),
//             properties: None,
//             name: None,
//             origin_time: None,
//         })
//         .await
//         .unwrap();
//     println!("clone_id_1 {}", clone_id_1);
//     assert_eq!(clone_id_1, format!("{}.{}", role_id, 1));
// }
