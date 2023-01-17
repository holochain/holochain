use crate::{
    conductor::{api::error::ConductorApiError, CellError},
    sweettest::*,
};
use holo_hash::ActionHash;
use holochain_conductor_api::CellInfo;
use holochain_types::{
    app::CreateCloneCellPayload,
    prelude::{CloneCellId, DeleteCloneCellPayload, DisableCloneCellPayload},
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{CloneId, DnaModifiersOpt, RoleName};

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_without_modifiers_fails() {
    let conductor = SweetConductor::from_standard_config().await;
    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "".to_string(),
            role_name: "".to_string(),
            modifiers: DnaModifiersOpt::none(),
            membrane_proof: None,
            name: None,
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_with_wrong_app_or_role_name_fails() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_name.clone(), dna)])
        .await
        .unwrap();

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: "wrong_app_id".to_string(),
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await;
    assert!(result.is_err());

    let result = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_name: "wrong_role_name".to_string(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_run_twice_returns_correct_clone_indexes() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_name.clone(), dna)])
        .await
        .unwrap();

    let installed_clone_cell_0 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed_1".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(
        installed_clone_cell_0.into_role_name(),
        *CloneId::new(&role_name, 0).as_app_role_name()
    ); // clone index starts at 0

    let installed_clone_cell_1 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed_2".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(
        installed_clone_cell_1.into_role_name(),
        *CloneId::new(&role_name, 1).as_app_role_name()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_creates_callable_cell() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_name.clone(), dna.clone())])
        .await
        .unwrap();

    let installed_clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
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
async fn clone_cell_deletion() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app_id = "app";
    conductor
        .setup_app_for_agent(app_id, alice.clone(), [&(role_name.clone(), dna)])
        .await
        .unwrap();
    let installed_clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app_id.to_string(),
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed_1".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();

    // disable clone cell
    conductor
        .raw_handle()
        .disable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.to_string(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.clone().into_role_name()).unwrap(),
            ),
        })
        .await
        .unwrap();

    // calling the cell after disabling fails
    let zome = SweetZome::new(
        installed_clone_cell.as_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_err());

    // enable the disabled clone cell by clone id
    let enabled_cell = conductor
        .raw_handle()
        .enable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.into(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.clone().into_role_name()).unwrap(),
            ),
        })
        .await
        .unwrap();

    // assert the enabled clone cell is the previously created clone cell
    assert_eq!(enabled_cell, installed_clone_cell);

    // assert that the cell appears in app info's cell data again
    let app_info = conductor
        .get_app_info(&app_id.to_string())
        .await
        .unwrap()
        .unwrap();
    let cell_info_for_role = app_info.cell_info.get(&role_name).unwrap();
    assert!(cell_info_for_role
        .iter()
        .find(|cell_info| if let CellInfo::Cloned(cell) = cell_info {
            cell.cell_id == installed_clone_cell.as_id().clone()
        } else {
            false
        })
        .is_some());

    // calling the cell after restoring succeeds
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());

    // disable clone cell again
    conductor
        .raw_handle()
        .disable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.to_string(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.clone().into_role_name()).unwrap(),
            ),
        })
        .await
        .unwrap();

    // enable clone cell by cell id
    let enabled_cell = conductor
        .raw_handle()
        .enable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.into(),
            clone_cell_id: CloneCellId::CellId(installed_clone_cell.as_id().clone()),
        })
        .await
        .unwrap();

    // assert the enabled clone cell is the previously created clone cell
    assert_eq!(enabled_cell, installed_clone_cell);

    // disable and delete clone cell
    conductor
        .raw_handle()
        .disable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.to_string(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.clone().into_role_name()).unwrap(),
            ),
        })
        .await
        .unwrap();
    conductor
        .raw_handle()
        .delete_clone_cell(&DeleteCloneCellPayload {
            app_id: app_id.into(),
            clone_cell_id: CloneCellId::CellId(installed_clone_cell.as_id().clone()),
        })
        .await
        .unwrap();
    // assert the deleted cell cannot be enabled
    let disable_result = conductor
        .raw_handle()
        .enable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.into(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.clone().into_role_name()).unwrap(),
            ),
        })
        .await;
    assert!(disable_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn conductor_can_startup_with_cloned_cell() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&(role_name.clone(), dna)])
        .await
        .unwrap();

    let clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed_1".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();

    // calling the cell works
    let zome = SweetZome::new(
        clone_cell.as_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());

    conductor.shutdown().await;
    conductor.startup().await;

    // calling the cell works after restart
    let zome = SweetZome::new(
        clone_cell.as_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());

    conductor
        .clone()
        .disable_clone_cell(&DisableCloneCellPayload {
            app_id: app.installed_app_id().clone(),
            clone_cell_id: CloneCellId::CellId(clone_cell.as_id().clone()),
        })
        .await
        .unwrap();

    // calling the cell after disabling fails
    let zome = SweetZome::new(
        clone_cell.as_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    matches!(zome_call_response, Err(ConductorApiError::CellError(CellError::CellDisabled(cell_id))) if cell_id == clone_cell.as_id().clone());

    conductor.shutdown().await;
    conductor.startup().await;

    // calling the cell still fails after restart, cell still disabled
    let zome = SweetZome::new(
        clone_cell.as_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    matches!(zome_call_response, Err(ConductorApiError::CellError(CellError::CellDisabled(cell_id))) if cell_id == clone_cell.as_id().clone());
}
