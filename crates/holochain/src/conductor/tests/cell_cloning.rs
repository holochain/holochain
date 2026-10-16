use crate::{
    conductor::{
        api::error::ConductorApiError,
        error::{CellUnavailableReason, ConductorError},
    },
    sweettest::*,
};
use holo_hash::ActionHash;
use holochain_conductor_api::{AppRequest, AppResponse, CellInfo};
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::init::InitProperties;
use matches::matches;
use std::sync::{Arc, Mutex};

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_without_modifiers_fails() {
    let conductor = SweetConductor::standard().await;
    let result = conductor
        .clone()
        .create_clone_cell(
            &"".into(),
            CreateCloneCellPayload {
                role_name: "".to_string(),
                modifiers: DnaModifiersOpt::none(),
                membrane_proof: None,
                name: None,
                init_properties: None,
            },
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_with_wrong_app_or_role_name_fails() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::standard().await;
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
                init_properties: None,
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
                init_properties: None,
            },
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_creates_callable_cell() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::standard().await;
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
                init_properties: None,
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
    let mut conductor = SweetConductor::standard().await;
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
                init_properties: None,
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
                init_properties: None,
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
    let mut conductor = SweetConductor::standard().await;
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
        init_properties: None,
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
    let mut conductor = SweetConductor::standard().await;
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
                init_properties: None,
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
    let mut conductor = SweetConductor::standard().await;
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
                init_properties: None,
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
    assert!(
        matches!(
            &zome_call_response,
            Err(ConductorApiError::ConductorError(ConductorError::CellNotRunning(
                cell_id,
                CellUnavailableReason::CloneCellDisabled
            ))) if *cell_id == clone_cell.cell_id
        ),
        "expected CloneCellDisabled, got: {zome_call_response:?}"
    );

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
    assert!(
        matches!(
            &zome_call_response,
            Err(ConductorApiError::ConductorError(ConductorError::CellNotRunning(
                cell_id,
                CellUnavailableReason::CloneCellDisabled
            ))) if *cell_id == clone_cell.cell_id
        ),
        "expected CloneCellDisabled after restart, got: {zome_call_response:?}"
    );
}

/// A DNA whose `init` records the value `get_init_properties` returned.
async fn init_recording_dna(seen: Arc<Mutex<Vec<Option<InitProperties>>>>) -> DnaFile {
    let zome = SweetInlineZomes::new(vec![], 0)
        .function("init", move |api, _: ()| {
            let props = api.get_init_properties(())?;
            seen.lock().unwrap().push(props);
            Ok(InitCallbackResult::Pass)
        })
        .function("touch", |_api, _: ()| Ok(()));
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;
    dna
}

fn init_props(bytes: Vec<u8>) -> InitProperties {
    InitProperties(SerializedBytes::from(UnsafeBytes::from(bytes)))
}

/// Init properties sent with a `CreateCloneCell` request over the app interface
/// are readable by the clone's `init`, are not visible to the provisioned cell,
/// and are removed once `init` has passed.
#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_with_init_properties_makes_them_available_in_init() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let dna = init_recording_dna(seen.clone()).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::standard().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna)])
        .await
        .unwrap();
    let props = init_props(vec![1, 2, 3]);

    let (app_ws, _app_ws_rx) = conductor
        .app_ws_client::<AppResponse>(app.installed_app_id().clone())
        .await;
    let response: AppResponse = app_ws
        .request(AppRequest::CreateCloneCell(Box::new(
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
                membrane_proof: None,
                name: None,
                init_properties: Some(props.clone()),
            },
        )))
        .await
        .unwrap();
    let AppResponse::CloneCellCreated(clone_cell) = response else {
        panic!("expected CloneCellCreated, got: {response:?}");
    };

    // Trigger init on the clone cell.
    let clone_zome = SweetZome::new(
        clone_cell.cell_id.clone(),
        SweetInlineZomes::COORDINATOR.into(),
    );
    let _: () = conductor.call(&clone_zome, "touch", ()).await;
    assert_eq!(seen.lock().unwrap().as_slice(), &[Some(props)]);

    // The row is removed once init has passed.
    let stored = conductor
        .raw_handle()
        .spaces
        .conductor_store
        .as_read()
        .get_init_properties(app.installed_app_id(), &clone_cell.clone_id.to_string())
        .await
        .unwrap();
    assert_eq!(stored, None);

    // The provisioned cell of the same role sees nothing.
    let base_zome = SweetZome::new(
        app.cells()[0].cell_id().clone(),
        SweetInlineZomes::COORDINATOR.into(),
    );
    let _: () = conductor.call(&base_zome, "touch", ()).await;
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        &[Some(init_props(vec![1, 2, 3])), None]
    );
}

/// Clones of the same role keep their init properties apart: each clone's `init`
/// sees only its own value, and deleting one clone leaves the others' rows intact.
#[tokio::test(flavor = "multi_thread")]
async fn clone_cell_init_properties_are_isolated_per_clone() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let dna = init_recording_dna(seen.clone()).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::standard().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna)])
        .await
        .unwrap();
    let app_id = app.installed_app_id().clone();
    let props_a = init_props(vec![13, 14, 15]);
    let props_c = init_props(vec![16, 17, 18]);

    let mut clones = Vec::new();
    for (seed, init_properties) in [
        ("a", Some(props_a.clone())),
        ("b", None),
        ("c", Some(props_c)),
    ] {
        let clone_cell = conductor
            .clone()
            .create_clone_cell(
                &app_id,
                CreateCloneCellPayload {
                    role_name: role_name.clone(),
                    modifiers: DnaModifiersOpt::none().with_network_seed(seed.to_string()),
                    membrane_proof: None,
                    name: None,
                    init_properties,
                },
            )
            .await
            .unwrap();
        clones.push(clone_cell);
    }
    let [clone_a, clone_b, clone_c] = clones.try_into().unwrap();

    // The clone created without init properties sees none, despite its siblings having some.
    let clone_b_zome = SweetZome::new(clone_b.cell_id, SweetInlineZomes::COORDINATOR.into());
    let _: () = conductor.call(&clone_b_zome, "touch", ()).await;
    assert_eq!(seen.lock().unwrap().as_slice(), &[None]);

    // Deleting clone C before its init ran removes only its own row.
    let clone_c_id = CloneCellId::CloneId(clone_c.clone_id.clone());
    conductor
        .clone()
        .disable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: clone_c_id.clone(),
            },
        )
        .await
        .unwrap();
    conductor
        .raw_handle()
        .delete_clone_cell(&DeleteCloneCellPayload {
            app_id: app_id.clone(),
            clone_cell_id: clone_c_id,
        })
        .await
        .unwrap();
    let store = conductor.raw_handle().spaces.conductor_store.as_read();
    assert_eq!(
        store
            .get_init_properties(&app_id, &clone_c.clone_id.to_string())
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .get_init_properties(&app_id, &clone_a.clone_id.to_string())
            .await
            .unwrap(),
        Some(props_a.clone())
    );

    // Clone A's init still receives its own value.
    let clone_a_zome = SweetZome::new(clone_a.cell_id, SweetInlineZomes::COORDINATOR.into());
    let _: () = conductor.call(&clone_a_zome, "touch", ()).await;
    assert_eq!(seen.lock().unwrap().as_slice(), &[None, Some(props_a)]);
}

/// When the clone's `init` fails, the init properties stay persisted so the
/// retried `init` on the next zome call still receives them.
#[tokio::test(flavor = "multi_thread")]
async fn clone_cell_init_properties_survive_failed_init() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_in_zome = seen.clone();
    let zome = SweetInlineZomes::new(vec![], 0)
        .function("init", move |api, _: ()| {
            let props = api.get_init_properties(())?;
            let mut seen = seen_in_zome.lock().unwrap();
            seen.push(props);
            if seen.len() == 1 {
                return Ok(InitCallbackResult::Fail("first init fails".into()));
            }
            Ok(InitCallbackResult::Pass)
        })
        .function("touch", |_api, _: ()| Ok(()));
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::standard().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna)])
        .await
        .unwrap();
    let app_id = app.installed_app_id().clone();
    let props = init_props(vec![7, 8, 9]);

    let clone_cell = conductor
        .clone()
        .create_clone_cell(
            &app_id,
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
                membrane_proof: None,
                name: None,
                init_properties: Some(props.clone()),
            },
        )
        .await
        .unwrap();
    let clone_key = clone_cell.clone_id.to_string();
    let clone_zome = SweetZome::new(
        clone_cell.cell_id.clone(),
        SweetInlineZomes::COORDINATOR.into(),
    );
    let store = conductor.raw_handle().spaces.conductor_store.as_read();

    let first: Result<(), _> = conductor.call_fallible(&clone_zome, "touch", ()).await;
    assert!(first.is_err());
    assert_eq!(
        store
            .get_init_properties(&app_id, &clone_key)
            .await
            .unwrap(),
        Some(props.clone())
    );

    let _: () = conductor.call(&clone_zome, "touch", ()).await;
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        &[Some(props.clone()), Some(props)]
    );
}

/// Clone init properties are stored under the clone id (not the role name), and stay
/// readable and removable while the clone is disabled, so an `init` that was already
/// running when the clone was disabled still receives them.
#[tokio::test(flavor = "multi_thread")]
async fn clone_cell_init_properties_resolve_while_disabled() {
    let (dna, _, _) =
        SweetDnaFile::unique_from_inline_zomes(SweetInlineZomes::new(vec![], 0)).await;
    let role_name: RoleName = "dna_1".to_string();
    let mut conductor = SweetConductor::standard().await;
    let app = conductor
        .setup_app("app", [&(role_name.clone(), dna)])
        .await
        .unwrap();
    let app_id = app.installed_app_id().clone();
    let props = init_props(vec![10, 11, 12]);

    let clone_cell = conductor
        .clone()
        .create_clone_cell(
            &app_id,
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
                membrane_proof: None,
                name: None,
                init_properties: Some(props.clone()),
            },
        )
        .await
        .unwrap();

    // The row is stored under the clone id, not the role name.
    let clone_key = clone_cell.clone_id.to_string();
    assert_eq!(clone_key, "dna_1.0");
    let store = conductor.raw_handle().spaces.conductor_store.as_read();
    assert_eq!(
        store
            .get_init_properties(&app_id, &clone_key)
            .await
            .unwrap(),
        Some(props.clone())
    );
    assert_eq!(
        store
            .get_init_properties(&app_id, &role_name)
            .await
            .unwrap(),
        None
    );

    conductor
        .clone()
        .disable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: CloneCellId::CloneId(clone_cell.clone_id.clone()),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        conductor
            .raw_handle()
            .get_init_properties_for_cell(&clone_cell.cell_id)
            .await
            .unwrap(),
        Some(props)
    );

    conductor
        .raw_handle()
        .delete_init_properties_for_cell(&clone_cell.cell_id)
        .await
        .unwrap();
    assert_eq!(
        conductor
            .raw_handle()
            .get_init_properties_for_cell(&clone_cell.cell_id)
            .await
            .unwrap(),
        None
    );
}
