use crate::sweettest::*;
use holochain_types::app::CreateCloneCellPayload;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{AppRoleId, CLONE_ID_DELIMITER};

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
    let role: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    conductor
        .setup_app_for_agent("app", alice.clone(), [&(role.clone(), dna)])
        .await
        .unwrap();

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "wrong_app_id".to_string(),
            role_id: role.clone(),
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
    let role: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role.clone(), dna)])
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
//     let role: AppRoleId = "dna_1".to_string();
//     let mut conductor = SweetConductor::from_standard_config().await;
//     let alice = SweetAgents::one(conductor.keystore()).await;
//     let app = conductor
//         .setup_app_for_agent("app", alice.clone(), [DnaWithRole {dna, role}])
//         .await
//         .unwrap();

//     let clone_id = conductor
//         .clone()
//         .create_clone_cell(CreateCloneCellPayload {
//             app_id: app.installed_app_id().clone(),
//             role_id: role.clone(),
//             network_seed: Some("seed".to_string()),
//             properties: None,
//             membrane_proof: None,
//             name: None,
//             origin_time: None,
//         })
//         .await
//         .unwrap();
//     conductor.call_zome(ZomeCall {,
//         /// The zome containing the function to be called
//         pub zome_name: ZomeName,
//         /// The name of the zome function to call
//         pub fn_name: FunctionName,
//         /// The serialized data to pass as an argument to the zome function call
//         pub payload: ExternIO,
//         /// The capability request authorization
//         ///
//         /// This can be `None` and still succeed in the case where the function
//         /// in the zome being called has been given an `Unrestricted` status
//         /// via a `CapGrant`. Otherwise it will be necessary to provide a `CapSecret` for every call.
//         pub cap_secret: Option<CapSecret>,
//         /// The provenance (source) of the call
//         ///
//         /// NB: **This will be removed** as soon as Holochain has a way of determining who
//         /// is making this zome call over this interface. Until we do, the caller simply
//         /// provides this data and Holochain trusts them.
//         pub provenance: AgentPubKey,})
//     assert_eq!(clone_id, format!("{}.{}", role_id, 0)); // clone index starts at 0
// }

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_returns_clone_id_with_correct_role_id() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
        .await
        .unwrap();
    let role: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role.clone(), dna)])
        .await
        .unwrap();

    let clone_id = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role.clone(),
            network_seed: Some("seed".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    assert_eq!(clone_id, format!("{}{}{}", role, CLONE_ID_DELIMITER, 0)); // clone index starts at 0
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_run_twice_returns_correct_clone_indexes() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
        .await
        .unwrap();
    let role: AppRoleId = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role.clone(), dna)])
        .await
        .unwrap();

    let clone_id_0 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role.clone(),
            network_seed: Some("seed_1".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    assert_eq!(clone_id_0, format!("{}{}{}", role, CLONE_ID_DELIMITER, 0)); // clone index starts at 0

    let clone_id_1 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_id: role.clone(),
            network_seed: Some("seed_2".to_string()),
            properties: None,
            membrane_proof: None,
            name: None,
            origin_time: None,
        })
        .await
        .unwrap();
    assert_eq!(clone_id_1, format!("{}{}{}", role, CLONE_ID_DELIMITER, 1));
}
