use holochain::sweettest::*;
use holochain_conductor_api::CellInfo;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell() {
    holochain_trace::test_run().unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let alice = SweetAgents::alice();

    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna_file])
        .await
        .unwrap();
    let (cell,) = app.clone().into_tuple();

    let zome = SweetZome::new(
        cell.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
        membrane_proof: None,
        name: Some("Clone 1".to_string()),
    };
    let _: ClonedCell = conductor.call(&zome, "create_clone", request).await;

    let apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(1, apps.len());

    let cell_infos = apps
        .first()
        .unwrap()
        .cell_info
        .get(&dna_file.dna_hash().to_string())
        .unwrap();
    assert_eq!(2, cell_infos.len());
    assert_eq!(
        1,
        cell_infos
            .into_iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn disable_enable_and_delete_clone_cell() {
    holochain_trace::test_run().unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let alice = SweetAgents::alice();

    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna_file])
        .await
        .unwrap();
    let (cell,) = app.clone().into_tuple();

    let zome = SweetZome::new(
        cell.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
        membrane_proof: None,
        name: Some("Clone 1".to_string()),
    };
    let cloned_cell: ClonedCell = conductor.call(&zome, "create_clone", request).await;

    let request = DisableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    let _: () = conductor.call(&zome, "disable_clone", request).await;

    // Try and call the disabled clone cell, should fail
    let clone_zome = SweetZome::new(
        cloned_cell.cell_id.clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 2".to_string()),
        membrane_proof: None,
        name: Some("Clone 2".to_string()),
    };
    conductor
        .call_fallible::<_, ClonedCell>(&clone_zome, "create_clone", request)
        .await
        .unwrap_err();

    // Re-enable the clone cell
    let request = EnableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    let _: ClonedCell = conductor.call(&zome, "enable_clone", request).await;

    // Try again to create a second clone cell, should succeed
    let request = CreateCloneCellInput {
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 2".to_string()),
        membrane_proof: None,
        name: Some("Clone 2".to_string()),
    };
    conductor
        .call::<_, ClonedCell>(&clone_zome, "create_clone", request)
        .await;

    // Now disable and delete the clone
    let request = DisableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    let _: () = conductor.call(&zome, "disable_clone", request).await;

    let request = DeleteCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    let _: () = conductor.call(&zome, "delete_clone", request).await;

    let apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(1, apps.len());

    let cell_infos = apps
        .first()
        .unwrap()
        .cell_info
        .get(&dna_file.dna_hash().to_string())
        .unwrap();
    // Still two cells, the original and the second clone
    assert_eq!(2, cell_infos.len());
    assert_eq!(
        1,
        cell_infos
            .into_iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn prevents_cross_app_clone_operations() {
    holochain_trace::test_run().unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let alice = SweetAgents::alice();

    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna_file])
        .await
        .unwrap();
    let (cell,) = app.clone().into_tuple();

    let zome = SweetZome::new(
        cell.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
        membrane_proof: None,
        name: Some("Clone 1".to_string()),
    };
    let cloned_cell: ClonedCell = conductor.call(&zome, "create_clone", request).await;

    let (other_dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;
    let other_app = conductor
        .setup_app_for_agent("other app", alice.clone(), [&other_dna_file])
        .await
        .unwrap();
    let (other_cell,) = other_app.clone().into_tuple();

    let apps = conductor.list_apps(None).await.unwrap();
    tracing::info!(?apps, "have apps");

    // Should fail to create a clone cell against the other app
    let other_zome = SweetZome::new(
        other_cell.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let other_request = CreateCloneCellInput {
        // Targeting the cell from the original app
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("other clone 1".to_string()),
        membrane_proof: None,
        name: Some("Other clone 1".to_string()),
    };
    conductor
        .call_fallible::<_, ClonedCell>(&other_zome, "create_clone", other_request)
        .await
        .unwrap_err();

    // Should fail to disable the clone cell from the other app
    let other_request = DisableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    conductor
        .call_fallible::<_, ()>(&other_zome, "disable_clone", other_request)
        .await
        .unwrap_err();

    // Actually disable the clone cell from the original app
    let request = DisableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    let _: () = conductor.call(&zome, "disable_clone", request).await;

    // Try to enable the clone cell from the other app, should fail
    let other_request = EnableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    conductor
        .call_fallible::<_, ClonedCell>(&other_zome, "enable_clone", other_request)
        .await
        .unwrap_err();

    // Try to delete the clone cell from the other app, should fail
    let other_request = DeleteCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    conductor
        .call_fallible::<_, ()>(&other_zome, "delete_clone", other_request)
        .await
        .unwrap_err();

    // Enable the cell again
    let request = EnableCloneCellInput {
        clone_cell_id: CloneCellId::CloneId(cloned_cell.clone_id.clone()),
    };
    let _: ClonedCell = conductor.call(&zome, "enable_clone", request).await;

    // Finally check the clone is still there
    let apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(2, apps.len());

    let cell_infos = apps
        .iter()
        .find(|app| app.installed_app_id == "app")
        .unwrap()
        .cell_info
        .get(&dna_file.dna_hash().to_string())
        .unwrap();
    assert_eq!(2, cell_infos.len());
    assert_eq!(
        1,
        cell_infos
            .into_iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_from_a_clone() {
    holochain_trace::test_run().unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let alice = SweetAgents::alice();

    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna_file])
        .await
        .unwrap();
    let (cell,) = app.clone().into_tuple();

    let zome = SweetZome::new(
        cell.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
        membrane_proof: None,
        name: Some("Clone 1".to_string()),
    };
    let cloned_cell: ClonedCell = conductor.call(&zome, "create_clone", request).await;

    let clone_zome = SweetZome::new(
        cloned_cell.cell_id.clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        // Clone the original cell
        cell_id: cell.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 2".to_string()),
        membrane_proof: None,
        name: Some("Clone 2".to_string()),
    };
    // Send the request to the clone cell
    let _: ClonedCell = conductor.call(&clone_zome, "create_clone", request).await;

    let apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(1, apps.len());

    let cell_infos = apps
        .first()
        .unwrap()
        .cell_info
        .get(&dna_file.dna_hash().to_string())
        .unwrap();
    assert_eq!(3, cell_infos.len());
    assert_eq!(
        2,
        cell_infos
            .into_iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_of_another_cell_in_same_app() {
    holochain_trace::test_run().unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file_1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;
    let (dna_file_2, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::DnaProperties]).await;

    let alice = SweetAgents::alice();

    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna_file_1, &dna_file_2])
        .await
        .unwrap();
    let (cell_1, cell_2) = app.clone().into_tuple();

    let zome = SweetZome::new(
        cell_1.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        // Try to clone cell 2 from cell 1
        cell_id: cell_2.cell_id().clone(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
        membrane_proof: None,
        name: Some("Clone 1".to_string()),
    };
    let _: ClonedCell = conductor.call(&zome, "create_clone", request).await;

    let apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(1, apps.len());

    let cell_infos = apps
        .first()
        .unwrap()
        .cell_info
        .get(&dna_file_2.dna_hash().to_string())
        .unwrap();
    assert_eq!(2, cell_infos.len());
    assert_eq!(
        1,
        cell_infos
            .into_iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}
