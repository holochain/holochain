use crate::sweettest::{SweetConductor, SweetDnaFile};
use hdk::prelude::{CellId, DnaModifiersOpt, NetworkSeed, RoleName};
use holochain_types::app::{
    AppBundle, AppBundleSource, AppManifest, AppManifestCurrentBuilder, AppRoleAssignment,
    AppRoleDnaManifest, AppRoleManifest, AppRolePrimary, AppStatus, CellProvisioning,
    DisabledAppReason, InstallAppPayload, InstalledApp, InstalledAppCommon, InstalledAppId,
    InstalledAppMap,
};
use holochain_types::dna::{DnaBundle, DnaFile};
use holochain_wasm_test_utils::TestWasm;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread")]
async fn can_update_state() {
    let db_dir = test_db_dir();
    let ribosome_store = RibosomeStore::new();
    let keystore = test_keystore();
    let holochain_p2p = holochain_p2p::stub_network().await;
    let (post_commit_sender, _post_commit_receiver) =
        tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);
    let config = ConductorConfig {
        data_root_path: Some(db_dir.path().to_path_buf().into()),
        ..Default::default()
    };
    let (outcome_tx, _outcome_rx) = futures::channel::mpsc::channel(8);
    let spaces = Spaces::new(
        config.clone().into(),
        Arc::new(Mutex::new(sodoken::LockedArray::from(
            b"passphrase".to_vec(),
        ))),
    )
    .await
    .unwrap();
    let conductor = Conductor::new(
        config.into(),
        ribosome_store,
        keystore,
        holochain_p2p,
        spaces,
        post_commit_sender,
        outcome_tx,
    );
    let state = conductor.get_state().await.unwrap();
    let mut expect_state = ConductorState::default();
    expect_state.set_tag(state.tag().clone());
    assert_eq!(state, expect_state);

    let cell_id = fake_cell_id(1);
    let installed_cell = InstalledCell::new(cell_id.clone(), "role_name".to_string());
    let app = InstalledAppCommon::new_legacy("fake app", vec![installed_cell]).unwrap();

    conductor
        .update_state(|mut state| {
            state.add_app(app)?;
            Ok(state)
        })
        .await
        .unwrap();
    let state = conductor.get_state().await.unwrap();
    assert_eq!(
        state.disabled_apps().map(second).collect::<Vec<_>>()[0]
            .all_cells()
            .collect::<Vec<_>>()
            .as_slice(),
        &[cell_id]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn can_set_fake_state() {
    let db_dir = test_db_dir();
    let expected = ConductorState::default();
    let conductor = ConductorBuilder::new()
        .config(SweetConductorConfig::standard().into())
        .fake_state(expected.clone())
        .with_data_root_path(db_dir.path().to_path_buf().into())
        .test(&[])
        .await
        .unwrap();
    let actual = conductor.get_state_from_handle().await.unwrap();
    assert_eq!(actual, expected);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "This kind of cell sharing is no longer possible.
            Keeping the test here to highlight the intention,
            though it will have to be removed or totally rewritten some day."]
async fn test_list_running_apps_for_dependent_cell_id() {
    holochain_trace::test_run();

    let mk_dna = |name: &'static str| async move {
        let zome = InlineIntegrityZome::new_unique(Vec::new(), 0);
        SweetDnaFile::unique_from_inline_zomes((name, zome)).await
    };

    // Create three unique DNAs
    let (dna1, _, _) = mk_dna("zome1").await;
    let (dna2, _, _) = mk_dna("zome2").await;
    let (dna3, _, _) = mk_dna("zome3").await;

    // Install two apps on the Conductor:
    // Both share a CellId in common, and also include a distinct CellId each.
    let mut conductor = SweetConductor::from_standard_config().await;
    let app1 = conductor.setup_app("app1", [&dna1, &dna2]).await.unwrap();
    let alice = app1.agent().clone();
    let app2 = conductor
        .setup_app_for_agent("app2", alice, [&dna1, &dna3])
        .await
        .unwrap();

    let (cell1, cell2) = app1.into_tuple();
    let (_, cell3) = app2.into_tuple();

    let list_apps = |conductor: ConductorHandle, cell: SweetCell| async move {
        conductor
            .list_enabled_apps_for_dependent_cell_id(cell.cell_id())
            .await
            .unwrap()
    };

    // - Ensure that the first CellId is associated with both apps,
    //   and the other two are only associated with one app each.
    assert_eq!(
        list_apps(conductor.clone(), cell1).await,
        hashset!["app1".to_string(), "app2".to_string()]
    );
    assert_eq!(
        list_apps(conductor.clone(), cell2).await,
        hashset!["app1".to_string()]
    );
    assert_eq!(
        list_apps(conductor.clone(), cell3).await,
        hashset!["app2".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn uninstall_app() {
    holochain_trace::test_run();
    let (dna, _, _) = mk_dna(simple_crud_zome()).await;
    let mut conductor = SweetConductor::from_standard_config().await;

    let app1 = conductor.setup_app("app1", [&dna]).await.unwrap();

    let hash1: ActionHash = conductor
        .call(
            &app1.cells()[0].zome("coordinator"),
            "create_string",
            "1".to_string(),
        )
        .await;

    let app2 = conductor.setup_app("app2", [&dna]).await.unwrap();

    let hash2: ActionHash = conductor
        .call(
            &app2.cells()[0].zome("coordinator"),
            "create_string",
            "1".to_string(),
        )
        .await;

    // Await integration of both actions.
    retry_fn_until_timeout(
        || async { conductor.all_ops_integrated(dna.dna_hash()).unwrap() },
        None,
        None,
    )
    .await
    .unwrap();

    assert!(conductor
        .call::<_, Option<Record>>(&app1.cells()[0].zome("coordinator"), "read", hash2.clone())
        .await
        .is_some());
    assert!(conductor
        .call::<_, Option<Record>>(&app2.cells()[0].zome("coordinator"), "read", hash1.clone())
        .await
        .is_some());

    // - Ensure that the apps are active
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.enabled_apps().count(), state.disabled_apps().count())
        },
        (2, 0)
    );

    let db1 = conductor
        .spaces
        .get_or_create_authored_db(dna.dna_hash(), app1.cells()[0].agent_pubkey().clone())
        .unwrap();
    let db2 = conductor
        .spaces
        .get_or_create_authored_db(dna.dna_hash(), app2.cells()[0].agent_pubkey().clone())
        .unwrap();

    // - Check that both authored database files exist
    std::fs::File::open(db1.path()).unwrap();
    std::fs::File::open(db2.path()).unwrap();

    // - Uninstall the first app
    conductor
        .raw_handle()
        .uninstall_app(&"app1".to_string(), false)
        .await
        .unwrap();

    // - Check that the first authored DB file is deleted since the cell was removed.
    #[cfg(not(windows))]
    std::fs::File::open(db1.path()).unwrap_err();
    std::fs::File::open(db2.path()).unwrap();

    // - Ensure that the remaining app can still access both hashes
    assert!(conductor
        .call::<_, Option<Record>>(&app2.cells()[0].zome("coordinator"), "read", hash1.clone())
        .await
        .is_some());
    assert!(conductor
        .call::<_, Option<Record>>(&app2.cells()[0].zome("coordinator"), "read", hash2.clone())
        .await
        .is_some());

    // - Uninstall the remaining app
    conductor
        .raw_handle()
        .uninstall_app(&"app2".to_string(), false)
        .await
        .unwrap();

    // - Check that second authored DB file is deleted since the cell was removed.
    #[cfg(not(windows))]
    std::fs::File::open(db2.path()).unwrap_err();

    // - Ensure that the apps are removed
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.enabled_apps().count(), state.disabled_apps().count())
        },
        (0, 0)
    );

    // - A new app can't read any of the data from the previous two, because once the last instance
    //   of the cells was destroyed, all data was destroyed as well.
    let app3 = conductor.setup_app("app2", [&dna]).await.unwrap();
    assert!(conductor
        .call::<_, Option<Record>>(&app3.cells()[0].zome("coordinator"), "read", hash1.clone())
        .await
        .is_none());
    assert!(conductor
        .call::<_, Option<Record>>(&app3.cells()[0].zome("coordinator"), "read", hash2.clone())
        .await
        .is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reconciliation_idempotency() {
    holochain_trace::test_run();
    let zome = InlineIntegrityZome::new_unique(Vec::new(), 0);
    let mut conductor = SweetConductor::from_standard_config().await;
    common_genesis_test_app(&mut conductor, ("custom", zome))
        .await
        .unwrap();

    conductor
        .raw_handle()
        .reconcile_cell_status_with_app_status()
        .await
        .unwrap();
    conductor
        .raw_handle()
        .reconcile_cell_status_with_app_status()
        .await
        .unwrap();

    // - Ensure that the app is active
    assert_eq_retry_10s!(conductor.list_enabled_apps().await.unwrap().len(), 1);
}
#[tokio::test(flavor = "multi_thread")]
async fn test_enable_disable_enable_app() {
    holochain_trace::test_run();
    let zome = simple_create_entry_zome();
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = common_genesis_test_app(&mut conductor, ("zome", zome))
        .await
        .unwrap();

    let all_apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(all_apps.len(), 1);

    let inactive_apps = conductor
        .list_apps(Some(AppStatusFilter::Disabled))
        .await
        .unwrap();
    let active_apps = conductor
        .list_apps(Some(AppStatusFilter::Enabled))
        .await
        .unwrap();
    assert_eq!(inactive_apps.len(), 0);
    assert_eq!(active_apps.len(), 1);
    assert_eq!(active_apps[0].cell_info.len(), 2);
    assert_matches!(active_apps[0].status, AppInfoStatus::Enabled);

    let (_, cell) = app.into_tuple();

    let hash: ActionHash = conductor
        .call_fallible(&cell.zome("zome"), "create", ())
        .await
        .unwrap();

    conductor
        .disable_app("app".to_string(), DisabledAppReason::User)
        .await
        .unwrap();

    let inactive_apps = conductor
        .list_apps(Some(AppStatusFilter::Disabled))
        .await
        .unwrap();
    let active_apps = conductor
        .list_apps(Some(AppStatusFilter::Enabled))
        .await
        .unwrap();
    assert_eq!(active_apps.len(), 0);
    assert_eq!(inactive_apps.len(), 1);
    assert_eq!(inactive_apps[0].cell_info.len(), 2);
    assert_matches!(
        inactive_apps[0].status,
        AppInfoStatus::Disabled {
            reason: DisabledAppReason::User
        }
    );

    // - We can't make a zome call while disabled
    assert!(conductor
        .call_fallible::<_, Option<Record>>(&cell.zome("zome"), "get", hash.clone())
        .await
        .is_err());

    conductor.enable_app("app".to_string()).await.unwrap();

    // - We can still make a zome call after reactivation
    assert!(conductor
        .call_fallible::<_, Option<Record>>(&cell.zome("zome"), "get", hash.clone())
        .await
        .is_ok());

    // - Ensure that the app is active

    assert_eq_retry_10s!(conductor.list_enabled_apps().await.unwrap().len(), 1);
    let inactive_apps = conductor
        .list_apps(Some(AppStatusFilter::Disabled))
        .await
        .unwrap();
    let active_apps = conductor
        .list_apps(Some(AppStatusFilter::Enabled))
        .await
        .unwrap();
    assert_eq!(active_apps.len(), 1);
    assert_eq!(inactive_apps.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_enable_disable_enable_clone_cell() {
    holochain_trace::test_run();
    let zome = simple_create_entry_zome();
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = common_genesis_test_app(&mut conductor, ("zome", zome))
        .await
        .unwrap();
    let app_id = app.installed_app_id().clone();

    let (clone, role_name) = {
        let (_, cell) = app.into_tuple();
        let role_name = cell.cell_id().dna_hash().to_string();

        let clone = conductor
            .create_clone_cell(
                &app_id,
                CreateCloneCellPayload {
                    role_name: role_name.clone(),
                    modifiers: DnaModifiersOpt::default().with_network_seed("new seed".into()),
                    membrane_proof: None,
                    name: None,
                },
            )
            .await
            .unwrap();

        (clone, role_name)
    };
    let zome = SweetZome::new(clone.cell_id.clone(), "zome".into());
    let hash: ActionHash = conductor.call(&zome, "create", ()).await;

    let clone_cell_id = CloneCellId::CloneId(clone.clone_id);
    conductor
        .disable_clone_cell(
            &app_id,
            &DisableCloneCellPayload {
                clone_cell_id: clone_cell_id.clone(),
            },
        )
        .await
        .unwrap();

    // - should not be able to call a zome fn on a disabled clone cell
    let result: Result<Option<Record>, _> =
        conductor.call_fallible(&zome, "get", hash.clone()).await;

    assert!(matches!(
        result,
        Err(ConductorApiError::ConductorError(
            ConductorError::CellDisabled(_)
        ))
    ));

    conductor.shutdown().await;
    conductor.startup().await;

    {
        // - cell should still be disabled after restart
        let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
        let cell = unwrap_cell_info_clone(app_info.cell_info.get(&role_name).unwrap()[1].clone());
        assert!(!cell.enabled);
    }
    {
        // - should *still* not be able to call a zome fn on a disabled clone cell after restart
        let result: Result<Option<Record>, _> =
            conductor.call_fallible(&zome, "get", hash.clone()).await;
        assert!(matches!(
            result,
            Err(ConductorApiError::ConductorError(
                ConductorError::CellDisabled(_)
            ))
        ));
    }

    conductor
        .raw_handle()
        .enable_clone_cell(&app_id, &EnableCloneCellPayload { clone_cell_id })
        .await
        .unwrap();

    {
        // - cell should still be enabled now
        let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
        let cell = unwrap_cell_info_clone(app_info.cell_info.get(&role_name).unwrap()[1].clone());
        assert!(cell.enabled);
    }
    {
        // - can call clone again
        let _: Option<Record> = conductor
            .call_fallible(&zome, "get", hash)
            .await
            .expect("can call zome fn now");
    }
}

// NB: currently the pre-genesis and post-genesis handling of panics is the same.
//   If we implement [ B-04188 ], then this test will be made more possible.
//   Otherwise, we have to devise a way to discover whether a panic happened
//   during genesis or not.
// NOTE: we need a test with a failure during a validation callback that happens
//       *inline*. It's not enough to have a failing validate for
//       instance, because that failure will be returned by the zome call.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to figure out how to write this test, i.e. to make genesis panic"]
async fn test_apps_disable_on_panic_after_genesis() {
    holochain_trace::test_run();
    let unit_entry_def = EntryDef::default_from_id("unit");
    let bad_zome =
        InlineZomeSet::new_unique_single("integrity", "custom", vec![unit_entry_def.clone()], 0)
            // We need a different validation callback that doesn't happen inline
            // so we can cause failure in it. But it must also be after genesis.
            .function("integrity", "validate", |_api, op: Op| {
                match op {
                    Op::StoreEntry(StoreEntry { action, .. })
                        if action.hashed.content.app_entry_def().is_some() =>
                    {
                        // Trigger a deserialization error
                        let _: Entry = SerializedBytes::try_from(())?.try_into()?;
                        Ok(ValidateResult::Valid)
                    }
                    _ => Ok(ValidateResult::Valid),
                }
            })
            .function("custom", "create", move |api, ()| {
                let entry = Entry::app(().try_into().unwrap()).unwrap();
                let hash = api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                    EntryVisibility::Public,
                    entry,
                    ChainTopOrdering::default(),
                ))?;
                Ok(hash)
            });

    let mut conductor = SweetConductor::from_standard_config().await;
    let app = common_genesis_test_app(&mut conductor, bad_zome)
        .await
        .unwrap();

    let (_, cell_bad) = app.into_tuple();

    let _: ConductorApiResult<ActionHash> = conductor
        .call_fallible(&cell_bad.zome("custom"), "create", ())
        .await;

    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.enabled_apps().count(), state.disabled_apps().count())
        },
        (0, 1)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_app_status_states() {
    holochain_trace::test_run();
    let zome = simple_create_entry_zome();
    let mut conductor = SweetConductor::from_standard_config().await;
    common_genesis_test_app(&mut conductor, ("zome", zome))
        .await
        .unwrap();

    let all_apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(all_apps.len(), 1);

    let get_status = || async { conductor.list_apps(None).await.unwrap()[0].status.clone() };

    // ENABLED  --disable->  DISABLED

    conductor
        .disable_app("app".to_string(), DisabledAppReason::User)
        .await
        .unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Disabled { .. });

    // DISABLED  --enable->  ENABLED

    conductor.enable_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Enabled);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "we don't have the ability to share cells across apps yet, but will need a test for that once we do"]
async fn test_app_status_states_multi_app() {
    todo!("write a test similar to the previous one, testing various state transitions, including switching on and off individual Cells");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_and_app_status_reconciliation() {
    holochain_trace::test_run();
    use AppStatusFx::*;
    use AppStatusKind::*;
    let mk_zome = || ("zome", InlineIntegrityZome::new_unique(Vec::new(), 0));
    let dnas = [
        mk_dna(mk_zome()).await.0,
        mk_dna(mk_zome()).await.0,
        mk_dna(mk_zome()).await.0,
    ];
    let app_id = "app".to_string();
    let config = SweetConductorConfig::standard();
    let mut conductor = SweetConductor::from_config(config).await;
    conductor.setup_app(&app_id, &dnas).await.unwrap();

    let cell_ids: Vec<_> = conductor.running_cell_ids().into_iter().collect();
    let cell1 = &cell_ids[0..1];

    let check = || async {
        (
            AppStatusKind::from(AppStatus::from(
                conductor.list_apps(None).await.unwrap()[0].status.clone(),
            )),
            conductor.running_cell_ids().len(),
        )
    };

    assert_eq!(check().await, (Enabled, 3));

    // - Simulate a cell being removed due to error
    conductor.remove_cells(cell1).await;
    assert_eq!(check().await, (Enabled, 2));

    // - App status won't change.
    let delta = conductor
        .reconcile_app_status_with_cell_status(None)
        .await
        .unwrap();
    assert_eq!(delta, NoChange);

    // - Disabling the app causes all cells to be removed
    conductor
        .disable_app(app_id.clone(), DisabledAppReason::User)
        .await
        .unwrap();
    assert_eq!(check().await, (Disabled, 0));

    // - ...but enabling one does
    conductor.enable_app(app_id).await.unwrap();
    assert_eq!(check().await, (Enabled, 3));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_app_status_filters() {
    holochain_trace::test_run();
    let zome = InlineIntegrityZome::new_unique(Vec::new(), 0);
    let dnas = [mk_dna(("dna", zome)).await.0];

    let mut conductor = SweetConductor::from_standard_config().await;

    conductor.setup_app("enabled", &dnas).await.unwrap();
    conductor.setup_app("disabled", &dnas).await.unwrap();

    // put apps in the proper states for testing

    conductor
        .disable_app("disabled".to_string(), DisabledAppReason::User)
        .await
        .unwrap();

    macro_rules! list_apps {
        ($filter: expr) => {
            conductor.list_apps($filter).await.unwrap()
        };
    }

    // Check the counts returned by each filter
    use AppStatusFilter::*;

    assert_eq!(list_apps!(None).len(), 2);
    assert_eq!(
        (
            list_apps!(Some(Enabled)).len(),
            list_apps!(Some(Disabled)).len(),
        ),
        (1, 1)
    );

    // check that counts are still accurate after a restart

    conductor.shutdown().await;
    conductor.startup().await;

    assert_eq!(list_apps!(None).len(), 2);
    assert_eq!(
        (
            list_apps!(Some(Enabled)).len(),
            list_apps!(Some(Disabled)).len(),
        ),
        (1, 1)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn app_operations() {
    let conductor = SweetConductor::from_standard_config().await;

    // Check conductor state is empty.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    assert_eq!(*state.installed_apps(), InstalledAppMap::new());

    // Running cells should be empty.
    conductor.running_cells.share_ref(|cells| {
        assert!(cells.is_empty());
    });

    // Install an app with 1 DNA.
    let app_id_1: InstalledAppId = "app_1".to_string();
    let role_1: RoleName = "role".to_string();
    let clone_limit_1 = 10;
    let App {
        bundle: bundle_1,
        dna_files: dna_files_1,
        manifest: manifest_1,
    } = make_app(
        app_id_1.clone(),
        app_id_1.clone(),
        [(vec![TestWasm::AgentInfo], role_1.clone())],
        clone_limit_1,
    )
    .await;
    let app_1 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bytes(bundle_1.pack().unwrap()),
            agent_key: None,
            installed_app_id: Some(app_id_1.clone()),
            network_seed: None,
            roles_settings: None,
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    // Check conductor state only contains the installed app.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    let mut expected_app_map = InstalledAppMap::new();
    expected_app_map.insert(
        app_id_1.clone(),
        InstalledApp::new(
            InstalledAppCommon::new(
                app_id_1.clone(),
                app_1.agent_key.clone(),
                [(
                    role_1.clone(),
                    AppRoleAssignment::Primary(AppRolePrimary::new(
                        dna_files_1[0].dna_hash().clone(),
                        true,
                        clone_limit_1,
                    )),
                )],
                manifest_1.clone(),
                app_1.installed_at,
            )
            .unwrap(),
            AppStatus::Disabled(DisabledAppReason::NeverStarted),
        ),
    );
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Enable app 1 now
    conductor.enable_app(app_id_1.clone()).await.unwrap();

    // Conductor state should reflect that the app is enabled.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.get_mut(&app_id_1).unwrap().status = AppStatus::Enabled;
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should only contain the installed cell.
    conductor.running_cells.share_ref(|cells| {
        assert_eq!(cells.len(), 1);
        let (cell_id, cell_item) = cells.get_index(0).unwrap();
        assert_eq!(
            *cell_id,
            CellId::new(dna_files_1[0].dna_hash().clone(), app_1.agent_key.clone())
        );
        assert_eq!(cell_id, cell_item.cell.id());
    });

    // Disable app
    conductor
        .disable_app(app_id_1.clone(), DisabledAppReason::User)
        .await
        .unwrap();

    // Conductor state should reflect that the app is disabled.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.get_mut(&app_id_1).unwrap().status =
        AppStatus::Disabled(DisabledAppReason::User);
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should again be empty.
    conductor.running_cells.share_ref(|cells| {
        assert!(cells.is_empty());
    });

    // Enable app again
    conductor.enable_app(app_id_1.clone()).await.unwrap();

    // Conductor state should reflect that the app is enabled.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.get_mut(&app_id_1).unwrap().status = AppStatus::Enabled;
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should contain only the installed cell.
    conductor.running_cells.share_ref(|cells| {
        assert_eq!(cells.len(), 1);
        let (cell_id, cell_item) = cells.get_index(0).unwrap();
        assert_eq!(
            *cell_id,
            CellId::new(dna_files_1[0].dna_hash().clone(), app_1.agent_key.clone())
        );
        assert_eq!(cell_id, cell_item.cell.id());
    });

    // Install another app with 2 DNAs
    let app_id_2: InstalledAppId = "app_2".to_string();
    let role_2_1: RoleName = "role_2_1".to_string();
    let role_2_2: RoleName = "role_2_2".to_string();
    let clone_limit_2 = 10;
    let App {
        bundle: bundle_2,
        dna_files: dna_files_2,
        manifest: manifest_2,
    } = make_app(
        app_id_2.clone(),
        app_id_2.clone(),
        [
            (vec![TestWasm::Foo], role_2_1.clone()),
            (vec![TestWasm::InitPass], role_2_2.clone()),
        ],
        clone_limit_2,
    )
    .await;
    let app_2 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bytes(bundle_2.pack().unwrap()),
            agent_key: None,
            installed_app_id: Some(app_id_2.clone()),
            network_seed: None,
            roles_settings: None,
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    // Enable app 2
    conductor.enable_app(app_id_2.clone()).await.unwrap();

    // Check conductor state contains both installed apps.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.insert(
        app_id_2.clone(),
        InstalledApp::new(
            InstalledAppCommon::new(
                app_id_2.clone(),
                app_2.agent_key.clone(),
                [
                    (
                        role_2_1.clone(),
                        AppRoleAssignment::Primary(AppRolePrimary::new(
                            dna_files_2[0].dna_hash().clone(),
                            true,
                            clone_limit_2,
                        )),
                    ),
                    (
                        role_2_2.clone(),
                        AppRoleAssignment::Primary(AppRolePrimary::new(
                            dna_files_2[1].dna_hash().clone(),
                            true,
                            clone_limit_2,
                        )),
                    ),
                ],
                manifest_2.clone(),
                app_2.installed_at,
            )
            .unwrap(),
            AppStatus::Enabled,
        ),
    );
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should contain cells of both apps.
    conductor.running_cells.share_ref(|cells| {
        assert_eq!(cells.len(), 3);
        // Cell of app 1
        let expected_cell_id =
            CellId::new(dna_files_1[0].dna_hash().clone(), app_1.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        // Cell 1 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[0].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        // Cell 2 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[1].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
    });

    // Uninstall app 1
    conductor
        .clone()
        .uninstall_app(&app_id_1, false)
        .await
        .unwrap();

    // Conductor state should reflect the app is gone.
    let state = conductor.get_state().await.unwrap();
    expected_app_map.shift_remove(&app_id_1);
    assert_eq!(state.app_interfaces, HashMap::new());
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should only contain only the cells of app 2.
    conductor.running_cells.share_ref(|cells| {
        // Cell 1 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[0].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        // Cell 2 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[1].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
    });

    // Uninstall app 2
    conductor
        .clone()
        .uninstall_app(&app_id_2, false)
        .await
        .unwrap();

    // Conductor state should reflect no more apps are installed.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    assert_eq!(*state.installed_apps(), InstalledAppMap::new());

    // Running cells should be empty.
    conductor.running_cells.share_ref(|cells| {
        assert!(cells.is_empty());
    });
}

struct App {
    bundle: AppBundle,
    dna_files: Vec<DnaFile>,
    manifest: AppManifest,
}

async fn make_app(
    app_id: InstalledAppId,
    network_seed: NetworkSeed,
    test_wasms_with_roles: impl IntoIterator<Item = (Vec<TestWasm>, RoleName)>,
    clone_limit: u32,
) -> App {
    let modifiers = DnaModifiersOpt::none().with_network_seed(network_seed.clone());
    let mut dna_files = Vec::new();
    let mut app_role_manifests = Vec::new();
    let mut resources = Vec::new();
    for (test_wasms, role) in test_wasms_with_roles {
        let dna_file =
            SweetDnaFile::from_test_wasms(network_seed.clone(), test_wasms, Default::default())
                .await
                .0;
        let dna_path = format!("{}", dna_file.dna_hash());
        let app_role_manifest = AppRoleManifest {
            name: role,
            dna: AppRoleDnaManifest {
                path: Some(dna_path.clone()),
                modifiers: modifiers.clone(),
                installed_hash: Some(dna_file.dna_hash().clone().into()),
                clone_limit,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        };
        app_role_manifests.push(app_role_manifest);
        let dna_bundle = DnaBundle::from_dna_file(dna_file.clone()).unwrap();
        resources.push((dna_path, dna_bundle));
        dna_files.push(dna_file);
    }
    let manifest: AppManifest = AppManifestCurrentBuilder::default()
        .name(app_id)
        .description(None)
        .roles(app_role_manifests)
        .allow_deferred_memproofs(false)
        .build()
        .unwrap()
        .into();
    let bundle = AppBundle::new(manifest.clone(), resources).unwrap();

    App {
        bundle,
        dna_files,
        manifest,
    }
}
