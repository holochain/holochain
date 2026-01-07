use super::Conductor;
use super::*;
use crate::conductor::api::error::ConductorApiError;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::sweettest::*;
use crate::test_utils::inline_zomes::simple_crud_zome;
use crate::{
    assert_eq_retry_10s, core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult,
};
use ::fixt::prelude::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holochain_conductor_api::CellInfo;
use holochain_keystore::crude_mock_keystore::*;
use holochain_keystore::test_keystore;
use holochain_types::{app::AppStatus, inline_zome::InlineZomeSet};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::op::Op;
use matches::assert_matches;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

mod add_agent_infos;
mod app_state;
mod cells_with_conflicting_overrides;
mod p2p_config_override;
mod state_dump;

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

    let app_id = "app_id".to_string();
    let app = InstalledAppCommon::new(
        app_id.clone(),
        fixt!(AgentPubKey),
        [(
            "handle".to_string(),
            AppRoleAssignment::Primary(AppRolePrimary::new(fixt!(DnaHash), true, 0)),
        )],
        AppManifest::V0(AppManifestV0 {
            allow_deferred_memproofs: false,
            description: None,
            name: "".to_string(),
            roles: vec![],
            bootstrap_url: None,
            signal_url: None,
        }),
        Timestamp::now(),
    )
    .unwrap();

    conductor.add_disabled_app_to_db(app.clone()).await.unwrap();

    assert_matches!(
        conductor.add_disabled_app_to_db(app.clone()).await,
        Err(ConductorError::AppAlreadyInstalled(id)) if id == app_id
    );
}

/// App can't be installed if it contains duplicate RoleNames
#[tokio::test(flavor = "multi_thread")]
async fn role_names_must_be_unique() {
    // Three unique role names succeed.
    let result = InstalledAppCommon::new(
        "id".to_string(),
        fixt!(AgentPubKey),
        [
            (
                "1".into(),
                AppRoleAssignment::Primary(AppRolePrimary::new(fixt!(DnaHash), true, 0)),
            ),
            (
                "2".into(),
                AppRoleAssignment::Primary(AppRolePrimary::new(fixt!(DnaHash), true, 0)),
            ),
            (
                "3".into(),
                AppRoleAssignment::Primary(AppRolePrimary::new(fixt!(DnaHash), true, 0)),
            ),
        ],
        AppManifest::V0(AppManifestV0 {
            name: "".to_string(),
            description: None,
            roles: vec![],
            allow_deferred_memproofs: false,
            bootstrap_url: None,
            signal_url: None,
        }),
        Timestamp::now(),
    );
    matches::assert_matches!(result, Ok(_));

    // Duplicate role names fail.
    let result = InstalledAppCommon::new(
        "id".to_string(),
        fixt!(AgentPubKey),
        [
            (
                "1".into(),
                AppRoleAssignment::Primary(AppRolePrimary::new(fixt!(DnaHash), true, 0)),
            ),
            (
                "1".into(),
                AppRoleAssignment::Primary(AppRolePrimary::new(fixt!(DnaHash), true, 0)),
            ),
            (
                "2".into(),
                AppRoleAssignment::Primary(AppRolePrimary::new(fixt!(DnaHash), true, 0)),
            ),
        ],
        AppManifest::V0(AppManifestV0 {
            name: "".to_string(),
            description: None,
            roles: vec![],
            allow_deferred_memproofs: false,
            bootstrap_url: None,
            signal_url: None,
        }),
        Timestamp::now(),
    );
    matches::assert_matches!(
        result,
        Err(AppError::DuplicateRoleNames(_, role_names)) if role_names == vec!["1".to_string()]
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
async fn test_signing_error_during_genesis() {
    holochain_trace::test_run();
    let bad_keystore = spawn_crude_mock_keystore(|| "spawn_crude_mock_keystore error".into()).await;

    let db_dir = test_db_dir();
    let config = ConductorConfig {
        data_root_path: Some(db_dir.path().to_path_buf().into()),
        ..Default::default()
    };
    let mut conductor = SweetConductor::new(
        SweetConductor::handle_from_existing(bad_keystore, &config, &[]).await,
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
        _ => panic!("wrong cell type: {cell_info:?}"),
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
            (state.enabled_apps().count(), state.disabled_apps().count())
        },
        (1, 0)
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
    assert_eq!(app_info.status, AppStatus::AwaitingMemproofs);
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
    conductor.startup(false).await;

    //- Status is still AwaitingMemproofs after a restart
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppStatus::AwaitingMemproofs);

    //- Status is still AwaitingMemproofs after enabling but before memproofs
    let r = conductor.enable_app(app_id.clone()).await;
    assert_matches!(r, Err(ConductorError::AppStatusError(_)));
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppStatus::AwaitingMemproofs);

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
        AppStatus::Disabled(DisabledAppReason::NotStartedAfterProvidingMemproofs)
    );

    conductor.enable_app(app_id.clone()).await.unwrap();

    //- Status is now Enabled and there is 1 cell assignment
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppStatus::Enabled);

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
