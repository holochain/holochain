use crate::{
    conductor::{api::error::ConductorApiError, error::ConductorError, CellError},
    sweettest::*,
};
use holo_hash::ActionHash;
use holochain_conductor_api::CellInfo;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use matches::matches;

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_without_modifiers_fails() {
    let conductor = SweetConductor::local_rendezvous().await;
    let result = conductor
        .clone()
        .create_clone_cell(
            &"".into(),
            CreateCloneCellPayload {
                role_name: "".to_string(),
                modifiers: DnaModifiersOpt::none(),
                membrane_proof: None,
                name: None,
            },
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_with_wrong_app_or_role_name_fails() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna)])
        .await
        .unwrap();

    let result = conductor
        .clone()
        .create_clone_cell(
            &"wrong_app_id".into(),
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
                membrane_proof: None,
                name: None,
            },
        )
        .await;
    assert!(result.is_err());

    let result = conductor
        .clone()
        .create_clone_cell(
            app.installed_app_id(),
            CreateCloneCellPayload {
                role_name: "wrong_role_name".to_string(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
                membrane_proof: None,
                name: None,
            },
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_creates_callable_cell() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna.clone())])
        .await
        .unwrap();

    let clone_name = "test_name".to_string();
    let clone_cell = conductor
        .clone()
        .create_clone_cell(
            app.installed_app_id(),
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
                membrane_proof: None,
                name: Some(clone_name.clone()),
            },
        )
        .await
        .unwrap();
    assert!(clone_cell.enabled);
    assert_eq!(clone_cell.name, clone_name);

    let zome = SweetZome::new(
        clone_cell.cell_id.clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_run_twice_returns_correct_clones() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna.clone())])
        .await
        .unwrap();

    let clone_cell_0 = conductor
        .clone()
        .create_clone_cell(
            app.installed_app_id(),
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed_1".to_string()),
                membrane_proof: None,
                name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(clone_cell_0.clone_id, CloneId::new(&role_name, 0)); // clone index starts at 0
    assert_eq!(clone_cell_0.original_dna_hash, dna.dna_hash().to_owned());

    let clone_cell_1 = conductor
        .clone()
        .create_clone_cell(
            app.installed_app_id(),
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed_2".to_string()),
                membrane_proof: None,
                name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(clone_cell_1.clone_id, CloneId::new(&role_name, 1));
    assert_eq!(clone_cell_1.original_dna_hash, dna.dna_hash().to_owned());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_identical_clone_cell_twice_fails() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::local_rendezvous().await;
    let apps = conductor
        .setup_apps("app", 2, [&(role_name.clone(), dna.clone())])
        .await
        .unwrap()
        .into_inner();
    let alice_app = &apps[0];
    let bob_app = &apps[1];
    let clone_cell_payload = CreateCloneCellPayload {
        role_name: role_name.clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
        membrane_proof: None,
        name: None,
    };

    let alice_clone_cell = conductor
        .clone()
        .create_clone_cell(alice_app.installed_app_id(), clone_cell_payload.clone())
        .await
        .unwrap();

    let identical_clone_cell_err = conductor
        .clone()
        .create_clone_cell(alice_app.installed_app_id(), clone_cell_payload.clone())
        .await;
    matches!(
        identical_clone_cell_err,
        Err(ConductorError::AppError(AppError::DuplicateCellId(cell_id))) if cell_id == alice_clone_cell.cell_id
    );

    // disable clone cell and try again to create an identical clone
    conductor
        .clone()
        .disable_clone_cell(
            alice_app.installed_app_id(),
            &DisableCloneCellPayload {
                clone_cell_id: CloneCellId::DnaHash(alice_clone_cell.cell_id.dna_hash().clone()),
            },
        )
        .await
        .unwrap();
    let identical_clone_cell_err = conductor
        .clone()
        .create_clone_cell(alice_app.installed_app_id(), clone_cell_payload.clone())
        .await;
    matches!(
        identical_clone_cell_err,
        Err(ConductorError::AppError(AppError::DuplicateCellId(cell_id))) if cell_id == alice_clone_cell.cell_id
    );

    // ensure that bob can clone cell with identical hash in same conductor
    let bob_clone_cell = conductor
        .clone()
        .create_clone_cell(bob_app.installed_app_id(), clone_cell_payload)
        .await
        .unwrap();
    assert_eq!(
        alice_clone_cell.original_dna_hash,
        bob_clone_cell.original_dna_hash
    );
    assert_eq!(
        alice_clone_cell.cell_id.dna_hash(),
        bob_clone_cell.cell_id.dna_hash()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn clone_cell_deletion() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::local_rendezvous().await;
    let app_id = "app".to_string();
    conductor
        .setup_app(&app_id, [&(role_name.clone(), dna)])
        .await
        .unwrap();
    let clone_cell = conductor
        .clone()
        .create_clone_cell(
            &app_id,
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed_1".to_string()),
                membrane_proof: None,
                name: None,
            },
        )
        .await
        .unwrap();
    let clone_id = CloneCellId::CloneId(clone_cell.clone().clone_id);

    // disable clone cell
    conductor
        .raw_handle()
        .disable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: clone_id.clone(),
            },
        )
        .await
        .unwrap();

    // calling the cell after disabling fails
    let zome = SweetZome::new(
        clone_cell.cell_id.clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_err());

    // enable the disabled clone cell by clone id
    let enabled_clone_cell = conductor
        .raw_handle()
        .enable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: clone_id.clone(),
            },
        )
        .await
        .unwrap();

    // assert the enabled clone cell is the previously created clone cell
    assert_eq!(enabled_clone_cell, clone_cell);

    // assert that the cell appears in app info's cell data again
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    let cell_info_for_role = app_info.cell_info.get(&role_name).unwrap();
    assert!(cell_info_for_role
        .iter()
        .any(|cell_info| if let CellInfo::Cloned(cell) = cell_info {
            cell.cell_id == clone_cell.cell_id.clone()
        } else {
            false
        }));

    // calling the cell after restoring succeeds
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());

    // disable clone cell again
    conductor
        .raw_handle()
        .disable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: clone_id.clone(),
            },
        )
        .await
        .unwrap();

    // enable clone cell by cell id
    let enabled_clone_cell = conductor
        .raw_handle()
        .enable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: CloneCellId::DnaHash(clone_cell.cell_id.dna_hash().clone()),
            },
        )
        .await
        .unwrap();

    // assert the enabled clone cell is the previously created clone cell
    assert_eq!(enabled_clone_cell, clone_cell);

    // disable and delete clone cell
    conductor
        .raw_handle()
        .disable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: clone_id.clone(),
            },
        )
        .await
        .unwrap();
    conductor
        .raw_handle()
        .delete_clone_cell(&DeleteCloneCellPayload {
            app_id: app_id.clone(),
            clone_cell_id: CloneCellId::DnaHash(clone_cell.cell_id.dna_hash().clone()),
        })
        .await
        .unwrap();
    // assert the deleted cell cannot be enabled
    let disable_result = conductor
        .raw_handle()
        .enable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: clone_id.clone(),
            },
        )
        .await;
    assert!(disable_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn conductor_can_startup_with_cloned_cell() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::local_rendezvous().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna)])
        .await
        .unwrap();

    let clone_cell = conductor
        .clone()
        .create_clone_cell(
            app.installed_app_id(),
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed_1".to_string()),
                membrane_proof: None,
                name: None,
            },
        )
        .await
        .unwrap();

    // calling the cell works
    let zome = SweetZome::new(
        clone_cell.cell_id.clone(),
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
        clone_cell.cell_id.clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    assert!(zome_call_response.is_ok());

    conductor
        .clone()
        .disable_clone_cell(
            app.installed_app_id(),
            &DisableCloneCellPayload {
                clone_cell_id: CloneCellId::DnaHash(clone_cell.cell_id.dna_hash().clone()),
            },
        )
        .await
        .unwrap();

    // calling the cell after disabling fails
    let zome = SweetZome::new(
        clone_cell.cell_id.clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    matches!(zome_call_response, Err(ConductorApiError::CellError(CellError::CellDisabled(cell_id))) if cell_id == clone_cell.cell_id.clone());

    conductor.shutdown().await;
    conductor.startup().await;

    // calling the cell still fails after restart, cell still disabled
    let zome = SweetZome::new(
        clone_cell.cell_id.clone(),
        TestWasm::Create.coordinator_zome_name(),
    );
    let zome_call_response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "call_create_entry", ())
        .await;
    matches!(zome_call_response, Err(ConductorApiError::CellError(CellError::CellDisabled(cell_id))) if cell_id == clone_cell.cell_id.clone());
}
