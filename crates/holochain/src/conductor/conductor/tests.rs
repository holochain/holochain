use super::Conductor;
use super::ConductorState;
use super::*;
use crate::conductor::api::error::ConductorApiError;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::sweettest::*;
use crate::test_utils::inline_zomes::simple_crud_zome;
use crate::test_utils::retry_fn_until_timeout;
use crate::{
    assert_eq_retry_10s, core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult,
};
use ::fixt::prelude::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holochain_conductor_api::AppInfoStatus;
use holochain_conductor_api::CellInfo;
use holochain_keystore::crude_mock_keystore::*;
use holochain_keystore::test_keystore;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::test_utils::fake_cell_id;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::op::Op;
use maplit::hashset;
use matches::assert_matches;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

mod add_agent_infos;
mod state_dump;

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
        state.stopped_apps().map(second).collect::<Vec<_>>()[0]
            .all_cells()
            .collect::<Vec<_>>()
            .as_slice(),
        &[cell_id]
    );
}

/// App can't be installed if another app is already installed under the
/// same InstalledAppId
#[tokio::test(flavor = "multi_thread")]
async fn app_ids_are_unique() {
    let db_dir = test_db_dir();
    let ribosome_store = RibosomeStore::new();
    let holochain_p2p = holochain_p2p::stub_network().await;
    let (post_commit_sender, _post_commit_receiver) =
        tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

    let (outcome_tx, _outcome_rx) = futures::channel::mpsc::channel(8);
    let config = ConductorConfig {
        data_root_path: Some(db_dir.path().to_path_buf().into()),
        ..Default::default()
    };
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
        test_keystore(),
        holochain_p2p,
        spaces,
        post_commit_sender,
        outcome_tx,
    );

    let cell_id = fake_cell_id(1);

    let installed_cell = InstalledCell::new(cell_id.clone(), "handle".to_string());
    let app = InstalledAppCommon::new_legacy("id".to_string(), vec![installed_cell]).unwrap();

    conductor.add_disabled_app_to_db(app.clone()).await.unwrap();

    assert_matches!(
        conductor.add_disabled_app_to_db(app.clone()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if id == *"id"
    );

    //- it doesn't matter whether the app is active or inactive
    let (_, delta) = conductor
        .transition_app_status("id".to_string(), AppStatusTransition::Enable)
        .await
        .unwrap();
    assert_eq!(delta, AppStatusFx::SpinUp);
    assert_matches!(
        conductor.add_disabled_app_to_db(app.clone()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if &id == "id"
    );
}

/// App can't be installed if it contains duplicate RoleNames
#[tokio::test(flavor = "multi_thread")]
async fn role_names_are_unique() {
    let agent = fixt!(AgentPubKey);
    let cells = vec![
        InstalledCell::new(CellId::new(fixt!(DnaHash), agent.clone()), "1".into()),
        InstalledCell::new(CellId::new(fixt!(DnaHash), agent.clone()), "1".into()),
        InstalledCell::new(CellId::new(fixt!(DnaHash), agent.clone()), "2".into()),
    ];
    let result = InstalledAppCommon::new_legacy("id", cells.into_iter());
    matches::assert_matches!(
        result,
        Err(AppError::DuplicateRoleNames(_, role_names)) if role_names == vec!["1".to_string()]
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
            .list_running_apps_for_dependent_cell_id(cell.cell_id())
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

async fn mk_dna(
    zomes: impl Into<InlineZomeSet>,
) -> (DnaFile, Vec<IntegrityZome>, Vec<CoordinatorZome>) {
    SweetDnaFile::unique_from_inline_zomes(zomes.into()).await
}

/// A function that sets up a SweetApp, used in several tests in this module
async fn common_genesis_test_app(
    conductor: &mut SweetConductor,
    custom_zomes: impl Into<InlineZomeSet>,
) -> ConductorApiResult<SweetApp> {
    let hardcoded_zome = InlineIntegrityZome::new_unique(Vec::new(), 0);

    // Create one DNA which always works, and another from a zome that gets passed in
    let (dna_hardcoded, _, _) = mk_dna(("hardcoded", hardcoded_zome)).await;
    let (dna_custom, _, _) = mk_dna(custom_zomes).await;

    // Install both DNAs under the same app:
    conductor
        .setup_app("app", &[dna_hardcoded, dna_custom])
        .await
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
            (state.running_apps().count(), state.stopped_apps().count())
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
            (state.running_apps().count(), state.stopped_apps().count())
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
    assert_eq_retry_10s!(conductor.list_running_apps().await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_signing_error_during_genesis() {
    holochain_trace::test_run();
    let bad_keystore = spawn_crude_mock_keystore(|| "spawn_crude_mock_keystore error".into()).await;

    let db_dir = test_db_dir();
    let config = ConductorConfig {
        data_root_path: Some(db_dir.path().to_path_buf().into()),
        ..Default::default()
    };
    let mut conductor = SweetConductor::new(
        SweetConductor::handle_from_existing(bad_keystore, &config, &[], false).await,
        db_dir.into(),
        config.into(),
        None,
    )
    .await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Sign]).await;

    let result = conductor
        .setup_app_for_agents("app", &[fixt!(AgentPubKey)], [&dna])
        .await;

    // - Assert that we got an error during Genesis. However, this test is
    //   pretty useless. What we really want is to ensure that the system is
    //   resilient when this type of error comes up in a real setting.
    let err = if let Err(err) = result {
        err
    } else {
        panic!("this should have been an error")
    };

    if let ConductorApiError::ConductorError(inner) = err {
        assert_matches!(inner, ConductorError::GenesisFailed { errors } if errors.len() == 1);
    } else {
        panic!("this should have been an error too");
    }
}

pub(crate) fn simple_create_entry_zome() -> InlineIntegrityZome {
    let unit_entry_def = EntryDef::default_from_id("unit");
    InlineIntegrityZome::new_unique(vec![unit_entry_def.clone()], 0)
        .function("create", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("get", move |api, hash: ActionHash| {
            let record = api
                .get(vec![GetInput::new(hash.into(), Default::default())])?
                .pop()
                .unwrap();

            Ok(record)
        })
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
    assert_matches!(active_apps[0].status, AppInfoStatus::Running);

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

    assert_eq_retry_10s!(conductor.list_running_apps().await.unwrap().len(), 1);
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

#[tokio::test(flavor = "multi_thread")]
async fn name_has_no_effect_on_dna_hash() {
    holochain_trace::test_run();
    let mut conductor = SweetConductor::from_standard_config().await;
    let dna = SweetDnaFile::unique_empty().await;
    let apps = conductor.setup_apps("app", 3, [&dna]).await.unwrap();
    let app_id1 = apps[0].installed_app_id().clone();
    let app_id2 = apps[1].installed_app_id().clone();
    let app_id3 = apps[2].installed_app_id().clone();
    let ((cell1,), (cell2,), (cell3,)) = apps.into_tuples();
    let role_name1 = cell1.cell_id().dna_hash().to_string();
    let role_name2 = cell2.cell_id().dna_hash().to_string();
    let role_name3 = cell3.cell_id().dna_hash().to_string();

    let clone1 = conductor
        .create_clone_cell(
            &app_id1,
            CreateCloneCellPayload {
                role_name: role_name1.clone(),
                modifiers: DnaModifiersOpt::default().with_network_seed("new seed".into()),
                membrane_proof: None,
                name: None,
            },
        )
        .await
        .unwrap();

    let clone2 = conductor
        .create_clone_cell(
            &app_id2,
            CreateCloneCellPayload {
                role_name: role_name2.clone(),
                modifiers: DnaModifiersOpt::default().with_network_seed("new seed".into()),
                membrane_proof: None,
                name: Some("Rumpelstiltskin".to_string()),
            },
        )
        .await
        .unwrap();

    let clone3 = conductor
        .create_clone_cell(
            &app_id3,
            CreateCloneCellPayload {
                role_name: role_name3.clone(),
                modifiers: DnaModifiersOpt::default().with_network_seed("new seed".into()),
                membrane_proof: None,
                name: Some("Chara".to_string()),
            },
        )
        .await
        .unwrap();

    assert_eq!(clone1.cell_id.dna_hash(), clone2.cell_id.dna_hash());
    assert_eq!(clone2.cell_id.dna_hash(), clone3.cell_id.dna_hash());
}

fn unwrap_cell_info_clone(cell_info: CellInfo) -> holochain_zome_types::clone::ClonedCell {
    match cell_info {
        CellInfo::Cloned(cell) => cell,
        _ => panic!("wrong cell type: {:?}", cell_info),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_installation_fails_if_genesis_self_check_is_invalid() {
    holochain_trace::test_run();
    let bad_zome = InlineZomeSet::new_unique_single("integrity", "custom", Vec::new(), 0).function(
        "integrity",
        "genesis_self_check",
        |_api, _data: GenesisSelfCheckData| {
            Ok(GenesisSelfCheckResult::Invalid(
                "intentional invalid result for testing".into(),
            ))
        },
    );

    let mut conductor = SweetConductor::from_standard_config().await;
    let err = if let Err(err) = common_genesis_test_app(&mut conductor, bad_zome).await {
        err
    } else {
        panic!("this should have been an error")
    };

    if let ConductorApiError::ConductorError(inner) = err {
        assert_matches!(inner, ConductorError::GenesisFailed { errors } if errors.len() == 1);
    } else {
        panic!("this should have been an error too");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_bad_entry_validation_after_genesis_returns_zome_call_error() {
    holochain_trace::test_run();
    let unit_entry_def = EntryDef::default_from_id("unit");
    let bad_zome =
        InlineZomeSet::new_unique_single("integrity", "custom", vec![unit_entry_def.clone()], 0)
            .function("integrity", "validate", |_api, op: Op| match op {
                Op::StoreEntry(StoreEntry { action, .. })
                    if action.hashed.content.app_entry_def().is_some() =>
                {
                    Ok(ValidateResult::Invalid(
                        "intentional invalid result for testing".into(),
                    ))
                }
                _ => Ok(ValidateResult::Valid),
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

    let mut conductor = SweetConductorConfig::standard().build_conductor().await;
    let app = common_genesis_test_app(&mut conductor, bad_zome)
        .await
        .unwrap();

    let (_, cell_bad) = app.into_tuple();

    let result: ConductorApiResult<ActionHash> = conductor
        .call_fallible(&cell_bad.zome("custom"), "create", ())
        .await;

    // - The failed validation simply causes the zome call to return an error
    assert_matches!(result, Err(_));

    // - The app is not disabled
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.running_apps().count(), state.stopped_apps().count())
        },
        (1, 0)
    );
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
            (state.running_apps().count(), state.stopped_apps().count())
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

    // RUNNING -pause-> PAUSED

    conductor
        .pause_app("app".to_string(), PausedAppReason::Error("because".into()))
        .await
        .unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Paused { .. });

    // PAUSED  --start->  RUNNING

    conductor.start_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Running);

    // RUNNING  --disable->  DISABLED

    conductor
        .disable_app("app".to_string(), DisabledAppReason::User)
        .await
        .unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Disabled { .. });

    // DISABLED  --start->  DISABLED

    conductor.start_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Disabled { .. });

    // DISABLED  --pause->  DISABLED

    conductor
        .pause_app("app".to_string(), PausedAppReason::Error("because".into()))
        .await
        .unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Disabled { .. });

    // DISABLED  --enable->  ENABLED

    conductor.enable_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Running);

    // RUNNING  --pause->  PAUSED

    conductor
        .pause_app("app".to_string(), PausedAppReason::Error("because".into()))
        .await
        .unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Paused { .. });

    // PAUSED  --enable->  RUNNING

    conductor.enable_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, AppInfoStatus::Running);
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

    assert_eq!(check().await, (Running, 3));

    // - Simulate a cell being removed due to error
    conductor.remove_cells(cell1).await;
    assert_eq!(check().await, (Running, 2));

    // - Again, app state should be reconciled to Paused due to missing cell
    let delta = conductor
        .reconcile_app_status_with_cell_status(None)
        .await
        .unwrap();
    assert_eq!(delta, SpinDown);
    assert_eq!(check().await, (Paused, 2));

    // - Disabling the app causes all cells to be removed
    conductor
        .disable_app(app_id.clone(), DisabledAppReason::User)
        .await
        .unwrap();
    assert_eq!(check().await, (Disabled, 0));

    // - Starting a disabled app does nothing
    conductor.start_app(app_id.clone()).await.unwrap();
    assert_eq!(check().await, (Disabled, 0));

    // - ...but enabling one does
    conductor.enable_app(app_id).await.unwrap();
    assert_eq!(check().await, (Running, 3));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_app_status_filters() {
    holochain_trace::test_run();
    let zome = InlineIntegrityZome::new_unique(Vec::new(), 0);
    let dnas = [mk_dna(("dna", zome)).await.0];

    let mut conductor = SweetConductor::from_standard_config().await;

    conductor.setup_app("running", &dnas).await.unwrap();
    conductor.setup_app("paused", &dnas).await.unwrap();
    conductor.setup_app("disabled", &dnas).await.unwrap();

    // put apps in the proper states for testing

    conductor
        .pause_app(
            "paused".to_string(),
            PausedAppReason::Error("because".into()),
        )
        .await
        .unwrap();

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

    assert_eq!(list_apps!(None).len(), 3);
    assert_eq!(
        (
            list_apps!(Some(Running)).len(),
            list_apps!(Some(Stopped)).len(),
            list_apps!(Some(Enabled)).len(),
            list_apps!(Some(Disabled)).len(),
            list_apps!(Some(Paused)).len(),
        ),
        (1, 2, 2, 1, 1,)
    );

    // check that paused apps move to Running state on conductor restart

    conductor.shutdown().await;
    conductor.startup().await;

    assert_eq!(list_apps!(None).len(), 3);
    assert_eq!(
        (
            list_apps!(Some(Running)).len(),
            list_apps!(Some(Stopped)).len(),
            list_apps!(Some(Enabled)).len(),
            list_apps!(Some(Disabled)).len(),
            list_apps!(Some(Paused)).len(),
        ),
        (2, 1, 2, 1, 0,)
    );
}

/// Check that the init() callback is only ever called once, even under many
/// concurrent initial zome function calls
#[tokio::test(flavor = "multi_thread")]
async fn test_init_concurrency() {
    holochain_trace::test_run();
    let num_inits = Arc::new(AtomicU32::new(0));
    let num_calls = Arc::new(AtomicU32::new(0));
    let num_inits_clone = num_inits.clone();
    let num_calls_clone = num_calls.clone();

    let zome = InlineZomeSet::new_unique_single("integrity", "zome", vec![], 0)
        .function("zome", "init", move |_, ()| {
            num_inits.clone().fetch_add(1, Ordering::SeqCst);
            Ok(InitCallbackResult::Pass)
        })
        .function("zome", "zomefunc", move |_, ()| {
            std::thread::sleep(std::time::Duration::from_millis(5));
            num_calls.clone().fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
    let dnas = [mk_dna(zome).await.0];
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor.setup_app("app", &dnas).await.unwrap();
    let (cell,) = app.into_tuple();
    let conductor = Arc::new(conductor);

    // Perform 100 concurrent zome calls
    let num_iters = Arc::new(AtomicU32::new(0));
    let call_tasks = (0..100_u32).map(|_i| {
        let conductor = conductor.clone();
        let zome = cell.zome("zome");
        let num_iters = num_iters.clone();
        tokio::spawn(async move {
            num_iters.fetch_add(1, Ordering::SeqCst);
            let _: () = conductor.call(&zome, "zomefunc", ()).await;
        })
    });
    let _ = futures::future::join_all(call_tasks).await;

    assert_eq!(num_iters.fetch_add(0, Ordering::SeqCst), 100);
    assert_eq!(num_calls_clone.fetch_add(0, Ordering::SeqCst), 100);
    assert_eq!(num_inits_clone.fetch_add(0, Ordering::SeqCst), 1);
}

/// Check that an app can be installed with deferred memproof provisioning and:
/// - all status checks return correctly while still provisioned,
/// - no zome calls can be made while awaiting memproofs,
/// - cells can be cloned while awaiting memproofs (even though this is unusual),
/// - conductor can be restarted and app still in AwaitingMemproofs state,
/// - app functions normally after memproofs provided
#[tokio::test(flavor = "multi_thread")]
async fn test_deferred_memproof_provisioning() {
    holochain_trace::test_run();
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app_id = "app-id".to_string();
    let role_name = "role".to_string();
    let bundle = app_bundle_from_dnas(&[(role_name.clone(), dna)], true, None).await;
    let bundle_bytes = bundle.pack().unwrap();

    //- Install with deferred memproofs
    let app = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bytes(bundle_bytes),
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            roles_settings: Default::default(),
            network_seed: None,
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    assert_eq!(app.role_assignments().len(), 1);

    let cell_id = app.all_cells().next().unwrap().clone();

    //- Status is AwaitingMemproofs and there is 1 cell assignment
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppInfoStatus::AwaitingMemproofs);
    assert_eq!(app_info.cell_info.len(), 1);

    let cell = conductor.get_sweet_cell(cell_id.clone()).unwrap();

    //- Can't make zome calls, error returned is CellDisabled
    //  (which isn't ideal, but gets the message across well enough)
    let result: Result<String, _> = conductor.call_fallible(&cell.zome("foo"), "foo", ()).await;
    assert_matches!(
        result,
        Err(ConductorApiError::ConductorError(
            ConductorError::CellDisabled(_)
        ))
    );

    conductor.shutdown().await;
    conductor.startup().await;

    //- Status is still AwaitingMemproofs after a restart
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppInfoStatus::AwaitingMemproofs);

    //- Status is still AwaitingMemproofs after enabling but before memproofs
    let r = conductor.enable_app(app_id.clone()).await;
    assert_matches!(r, Err(ConductorError::AppStatusError(_)));
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppInfoStatus::AwaitingMemproofs);

    //- Can not create a clone cell until memproofs have been provided
    let error = conductor
        .create_clone_cell(
            &app_id,
            CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seeeeed".into()),
                membrane_proof: None,
                name: None,
            },
        )
        .await
        .unwrap_err();
    assert_matches!(
        error,
        ConductorError::SourceChainError(SourceChainError::ChainEmpty)
    );

    //- Now provide the memproofs
    conductor
        .clone()
        .provide_memproofs(&app_id, MemproofMap::new())
        .await
        .unwrap();

    //- Status is now Disabled with the special `NotStartedAfterProvidingMemproofs` reason.
    //    It's not tested in this test, but this status allows the app to be enabled
    //    over the app interface.
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(
        app_info.status,
        AppInfoStatus::Disabled {
            reason: DisabledAppReason::NotStartedAfterProvidingMemproofs
        }
    );

    conductor.enable_app(app_id.clone()).await.unwrap();

    //- Status is now Running and there is 1 cell assignment
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppInfoStatus::Running);

    //- And now we can make a zome call successfully
    let _: String = conductor.call(&cell.zome("foo"), "foo", ()).await;

    //- And create a clone cell
    conductor
        .create_clone_cell(
            &app_id,
            CreateCloneCellPayload {
                role_name,
                modifiers: DnaModifiersOpt::none().with_network_seed("seeeeed".into()),
                membrane_proof: None,
                name: None,
            },
        )
        .await
        .unwrap();
}

/// Can uninstall an app with deferred memproofs before providing memproofs
#[tokio::test(flavor = "multi_thread")]
async fn test_deferred_memproof_provisioning_uninstall() {
    holochain_trace::test_run();
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;
    let conductor = SweetConductor::from_standard_config().await;
    let app_id = "app-id".to_string();
    let role_name = "role".to_string();
    let bundle = app_bundle_from_dnas(&[(role_name.clone(), dna)], true, None).await;
    let bundle_bytes = bundle.pack().unwrap();

    //- Install with deferred memproofs
    conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bytes(bundle_bytes),
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            roles_settings: Default::default(),
            network_seed: None,
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    assert_eq!(conductor.list_apps(None).await.unwrap().len(), 1);
    conductor
        .clone()
        .uninstall_app(&app_id, false)
        .await
        .unwrap();
    assert_eq!(conductor.list_apps(None).await.unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_apps_sorted_consistently() {
    holochain_trace::test_run();

    // Create a DNA
    let zome = InlineIntegrityZome::new_unique(Vec::new(), 0);
    let (dna1, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome1", zome)).await;

    // Install two apps on the Conductor:
    // Both share a CellId in common, and also include a distinct CellId each.
    let mut conductor = SweetConductor::from_standard_config().await;
    let _ = conductor.setup_app("app1", [&dna1]).await.unwrap();
    let _ = conductor.setup_app("app2", [&dna1]).await.unwrap();
    let _ = conductor.setup_app("app3", [&dna1]).await.unwrap();

    let list_app_ids = |conductor: ConductorHandle| async move {
        conductor
            .list_apps(None)
            .await
            .unwrap()
            .into_iter()
            .map(|app_info| app_info.installed_app_id)
            .collect::<Vec<String>>()
    };

    // Ensure that ordering is sorted by installed_at descending
    assert_eq!(
        list_app_ids(conductor.clone()).await,
        ["app3".to_string(), "app2".to_string(), "app1".to_string()]
    );

    // Ensure that ordering is consistent every time
    assert_eq!(
        list_app_ids(conductor.clone()).await,
        ["app3".to_string(), "app2".to_string(), "app1".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_app_info_cells_sorted_consistently() {
    holochain_trace::test_run();

    // Create a DNA
    let zome = InlineIntegrityZome::new_unique(Vec::new(), 0);
    let (dna1, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome1", zome.clone())).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome1", zome.clone())).await;
    let (dna3, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome1", zome)).await;

    // Install app on the Conductor:
    let mut conductor = SweetConductor::from_standard_config().await;
    let _ = conductor
        .setup_app(
            "app1",
            [
                &("dna1".to_string(), dna1),
                &("dna2".to_string(), dna2),
                &("dna3".to_string(), dna3),
            ],
        )
        .await
        .unwrap();

    let get_app_info = |conductor: ConductorHandle| async move {
        conductor
            .get_app_info(&"app1".to_string())
            .await
            .expect("Failed to get app info")
            .unwrap()
            .cell_info
    };

    // Ensure that ordering is sorted
    assert_eq!(
        get_app_info(conductor.clone())
            .await
            .keys()
            .collect::<Vec<&String>>(),
        vec![
            &"dna1".to_string(),
            &"dna2".to_string(),
            &"dna3".to_string()
        ]
    );

    // Ensure that ordering is consistent every time
    assert_eq!(
        get_app_info(conductor.clone())
            .await
            .keys()
            .collect::<Vec<&String>>(),
        vec![
            &"dna1".to_string(),
            &"dna2".to_string(),
            &"dna3".to_string()
        ]
    );
}
