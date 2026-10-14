use holochain::conductor::error::ConductorError;
use holochain::sweettest::*;
use holochain_conductor_api::CellInfo;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
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
            .iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn disable_enable_and_delete_clone_cell() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
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
            .iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(feature = "wasmer-wasmi", ignore)]
async fn prevents_cross_app_clone_operations() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
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
        .setup_app("other app", [&other_dna_file])
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
            .iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell_from_a_clone() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
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
            .iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_of_another_cell_in_same_app() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let (dna_file_1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;
    let (dna_file_2, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::DnaProperties]).await;

    let app = conductor
        .setup_app("app", [&dna_file_1, &dna_file_2])
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
            .iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}

// Reproduction: `create_clone_cell` adds the clone cell to the conductor state
// before it runs genesis, and does not remove it again when genesis fails.
// The conductor state then contains a cell without genesis records, and every
// later conductor startup fails with `CellWithoutGenesis`.
#[tokio::test(flavor = "multi_thread")]
async fn conductor_restarts_after_clone_cell_genesis_failure() {
    holochain_trace::test_run();

    // Genesis succeeds without a membrane proof and fails with one. This makes
    // it possible to install the app and then fail genesis for a clone cell.
    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function(
        "genesis_self_check_2",
        |_api, data: GenesisSelfCheckDataV2| {
            Ok(match data.membrane_proof {
                None => ValidateCallbackResult::Valid,
                Some(_) => ValidateCallbackResult::Invalid("no membrane proof allowed".into()),
            })
        },
    );
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    let mut conductor = SweetConductor::standard().await;
    conductor.setup_app("app", [&dna_file]).await.unwrap();

    let result = conductor
        .raw_handle()
        .create_clone_cell(
            &"app".to_string(),
            CreateCloneCellPayload {
                role_name: dna_file.dna_hash().to_string(),
                modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
                membrane_proof: Some(MembraneProof::new(SerializedBytes::from(
                    UnsafeBytes::from(vec![0]),
                ))),
                name: Some("Clone 1".to_string()),
            },
        )
        .await;
    assert!(
        matches!(result, Err(ConductorError::GenesisFailed { .. })),
        "expected genesis to fail, got {result:?}"
    );

    conductor.shutdown().await;

    // Panics with `CellWithoutGenesis` for the clone cell.
    conductor.startup().await;

    assert_eq!(1, conductor.list_apps(None).await.unwrap().len());
}

// Two simultaneous `create_clone_cell` calls for the identical clone cell must
// create exactly one clone cell with one valid source chain, and the conductor
// must be able to restart.
#[tokio::test(flavor = "multi_thread")]
async fn create_identical_clone_cell_concurrently() {
    holochain_trace::test_run();

    let zomes = SweetInlineZomes::new(vec![], 0)
        // Slow genesis down, so that both calls are in progress at the same time.
        .integrity_function("genesis_self_check_2", |_api, _: GenesisSelfCheckDataV2| {
            std::thread::sleep(std::time::Duration::from_millis(500));
            Ok(ValidateCallbackResult::Valid)
        })
        .function("chain", |api, _: ()| {
            api.query(ChainQueryFilter::new()).map_err(Into::into)
        });
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
    let (cell,) = app.into_tuple();

    let payload = CreateCloneCellPayload {
        role_name: dna_file.dna_hash().to_string(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
        membrane_proof: None,
        name: Some("Clone 1".to_string()),
    };
    let create = |payload: CreateCloneCellPayload| {
        let handle = conductor.raw_handle();
        tokio::spawn(async move { handle.create_clone_cell(&"app".to_string(), payload).await })
    };
    let (result_1, result_2) = tokio::join!(create(payload.clone()), create(payload));
    let (created, failed): (Vec<_>, Vec<_>) = [result_1.unwrap(), result_2.unwrap()]
        .into_iter()
        .partition(|result| result.is_ok());
    assert_eq!(1, created.len(), "expected one call to succeed: {failed:?}");
    assert_eq!(1, failed.len());
    let cloned_cell = created.into_iter().next().unwrap().unwrap();

    let clone_zome = SweetZome::new(
        cloned_cell.cell_id.clone(),
        SweetInlineZomes::COORDINATOR.into(),
    );
    let base_zome = cell.zome(SweetInlineZomes::COORDINATOR);

    for _ in 0..2 {
        let apps = conductor.list_apps(None).await.unwrap();
        let cell_infos = apps[0]
            .cell_info
            .get(&dna_file.dna_hash().to_string())
            .unwrap();
        assert_eq!(
            1,
            cell_infos
                .iter()
                .filter(|c| matches!(c, CellInfo::Cloned(_)))
                .count()
        );

        // The source chain of the clone cell has one record for each sequence
        // number, and the same length as the source chain of the base cell.
        let base_chain: Vec<Record> = conductor.call(&base_zome, "chain", ()).await;
        let clone_chain: Vec<Record> = conductor.call(&clone_zome, "chain", ()).await;
        let seqs: Vec<u32> = clone_chain
            .iter()
            .map(|r| r.action().action_seq())
            .collect();
        assert_eq!((0..base_chain.len() as u32).collect::<Vec<_>>(), seqs);

        // Do the same checks again after a restart.
        conductor.shutdown().await;
        conductor.startup().await;
    }
}
