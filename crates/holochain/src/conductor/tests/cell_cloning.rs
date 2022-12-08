use crate::sweettest::*;
use holo_hash::ActionHash;
use holochain_types::{
    app::CreateCloneCellPayload,
    prelude::{DisableCloneCellPayload, CloneCellId, DeleteArchivedCloneCellsPayload},
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
async fn app_info_includes_cloned_cells() {
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

    let app_info = conductor
        .get_app_info(&app_id.to_string())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(app_info.cell_data.len(), 2);
    assert!(app_info.cell_data.contains(&installed_clone_cell));
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

    // archive clone cell
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

    // assert that the cell doesn't appear any longer in app info's cell data
    let app_info = conductor
        .get_app_info(&app_id.to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app_info.cell_data.contains(&installed_clone_cell), false);

    // calling the cell after archiving fails
    let zome = SweetZome::new(
        installed_clone_cell.as_id().clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_err());

    // restore the archived clone cell by clone id
    let restored_cell = conductor
        .raw_handle()
        .restore_archived_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.into(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.clone().into_role_name()).unwrap(),
            ),
        })
        .await
        .unwrap();

    // assert the restored clone cell is the previously created clone cell
    assert_eq!(restored_cell, installed_clone_cell);

    // assert that the cell appears in app info's cell data again
    let app_info = conductor
        .get_app_info(&app_id.to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app_info.cell_data.contains(&installed_clone_cell), true);

    // calling the cell after restoring succeeds
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());

    // archive clone cell again
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

    // restore clone cell by cell id
    let restored_cell = conductor
        .raw_handle()
        .restore_archived_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.into(),
            clone_cell_id: CloneCellId::CellId(installed_clone_cell.as_id().clone()),
        })
        .await
        .unwrap();

    // assert the restored clone cell is the previously created clone cell
    assert_eq!(restored_cell, installed_clone_cell);

    // archive and delete clone cell
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
        .delete_archived_clone_cells(&DeleteArchivedCloneCellsPayload {
            app_id: app_id.into(),
            role_name: role_name.clone(),
        })
        .await
        .unwrap();
    // assert the deleted cell cannot be restored
    let restore_result = conductor
        .raw_handle()
        .restore_archived_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.into(),
            clone_cell_id: CloneCellId::CloneId(
                CloneId::try_from(installed_clone_cell.clone().into_role_name()).unwrap(),
            ),
        })
        .await;
    assert!(restore_result.is_err());
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

    let _ = conductor
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

    conductor.shutdown().await;
    conductor.startup().await;

    // Simply test that the conductor can startup with a cloned cell present.
}
