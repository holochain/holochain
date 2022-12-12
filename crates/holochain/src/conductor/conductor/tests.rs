use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use super::Conductor;
use super::ConductorState;
use super::*;
use crate::conductor::api::error::ConductorApiError;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::sweettest::*;
use crate::{
    assert_eq_retry_10s, core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult,
};
use ::fixt::prelude::*;
use holochain_conductor_api::InstalledAppInfoStatus;
use holochain_keystore::crude_mock_keystore::*;
use holochain_state::nonce::fresh_nonce;
use holochain_state::prelude::test_keystore;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::test_utils::fake_cell_id;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::op::Op;
use maplit::hashset;
use matches::assert_matches;

#[tokio::test(flavor = "multi_thread")]
async fn can_update_state() {
    let db_dir = test_db_dir();
    let ribosome_store = RibosomeStore::new();
    let keystore = test_keystore();
    let holochain_p2p = holochain_p2p::stub_network().await;
    let (post_commit_sender, _post_commit_receiver) =
        tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);
    let spaces = Spaces::new(&ConductorConfig {
        environment_path: db_dir.path().to_path_buf().into(),
        ..Default::default()
    })
    .unwrap();
    let conductor = Conductor::new(
        Default::default(),
        ribosome_store,
        keystore,
        holochain_p2p,
        spaces,
        post_commit_sender,
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
        &[&cell_id]
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
    let spaces = Spaces::new(&ConductorConfig {
        environment_path: db_dir.path().to_path_buf().into(),
        ..Default::default()
    })
    .unwrap();
    let conductor = Conductor::new(
        Default::default(),
        ribosome_store,
        test_keystore(),
        holochain_p2p,
        spaces,
        post_commit_sender,
    );

    let cell_id = fake_cell_id(1);

    let installed_cell = InstalledCell::new(cell_id.clone(), "handle".to_string());
    let app = InstalledAppCommon::new_legacy("id".to_string(), vec![installed_cell]).unwrap();

    conductor
        .add_disabled_app_to_db(app.clone().into())
        .await
        .unwrap();

    assert_matches!(
        conductor.add_disabled_app_to_db(app.clone().into()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if id == "id".to_string()
    );

    //- it doesn't matter whether the app is active or inactive
    let (_, delta) = conductor
        .transition_app_status("id".to_string(), AppStatusTransition::Enable)
        .await
        .unwrap();
    assert_eq!(delta, AppStatusFx::SpinUp);
    assert_matches!(
        conductor.add_disabled_app_to_db(app.clone().into()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if id == "id".to_string()
    );
}

/// App can't be installed if it contains duplicate RoleNames
#[tokio::test(flavor = "multi_thread")]
async fn role_names_are_unique() {
    let cells = vec![
        InstalledCell::new(fixt!(CellId), "1".into()),
        InstalledCell::new(fixt!(CellId), "1".into()),
        InstalledCell::new(fixt!(CellId), "2".into()),
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
    let state = ConductorState::default();
    let conductor = ConductorBuilder::new()
        .fake_state(state.clone())
        .test(db_dir.path(), &[])
        .await
        .unwrap();
    assert_eq!(state, conductor.get_state_from_handle().await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_running_apps_for_dependent_cell_id() {
    observability::test_run().ok();

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
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app1 = conductor
        .setup_app_for_agent("app1", alice.clone(), [&dna1, &dna2])
        .await
        .unwrap();
    let app2 = conductor
        .setup_app_for_agent("app2", alice.clone(), [&dna1, &dna3])
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

    // Just a strong reminder that we need to be careful once we start using existing Cells:
    // When a Cell panics or fails validation in general, we want to disable all Apps touching that Cell.
    // However, if the panic/failure happens during Genesis, we want to completely
    // destroy the app which is attempting to Create that Cell, but *NOT* any other apps
    // which might be touching that Cell.
    //
    // It probably works out to be the same either way, since if we are creating a Cell,
    // no other app could be possibly referencing it, but just in case we have some kind of complex
    // behavior like installing two apps which reference each others' Cells at the same time,
    // we need to be aware of this distinction.
    holochain_types::app::we_must_remember_to_rework_cell_panic_handling_after_implementing_use_existing_cell_resolution(
    );

    // Create one DNA which always works, and another from a zome that gets passed in
    let (dna_hardcoded, _, _) = mk_dna(("hardcoded", hardcoded_zome)).await;
    let (dna_custom, _, _) = mk_dna(custom_zomes).await;

    // Install both DNAs under the same app:
    conductor
        .setup_app(&"app", &[dna_hardcoded, dna_custom])
        .await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_uninstall_app() {
    observability::test_run().ok();
    let zome = InlineIntegrityZome::new_unique(Vec::new(), 0);
    let mut conductor = SweetConductor::from_standard_config().await;
    common_genesis_test_app(&mut conductor, ("custom", zome))
        .await
        .unwrap();

    // - Ensure that the app is active
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.running_apps().count(), state.stopped_apps().count())
        },
        (1, 0)
    );

    conductor
        .raw_handle()
        .uninstall_app(&"app".to_string())
        .await
        .unwrap();

    // - Ensure that the app is removed
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.running_apps().count(), state.stopped_apps().count())
        },
        (0, 0)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reconciliation_idempotency() {
    observability::test_run().ok();
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
    observability::test_run().ok();
    let bad_keystore = spawn_crude_mock_keystore(|| "test error".into()).await;

    let db_dir = test_db_dir();
    let config = ConductorConfig::default();
    let mut conductor = SweetConductor::new(
        SweetConductor::handle_from_existing(db_dir.path(), bad_keystore, &config, &[]).await,
        db_dir.into(),
        config,
    )
    .await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Sign]).await;

    let result = conductor
        .setup_app_for_agents(&"app", &[fixt!(AgentPubKey)], &[dna])
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

async fn make_signing_call(
    conductor: &SweetConductor,
    client: &mut WebsocketSender,
    keystore_control: &MockLairControl,
    cell: &SweetCell,
) -> AppResponse {
    let reinstate_mock = keystore_control.using_mock();
    if reinstate_mock {
        keystore_control.use_real();
    }
    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    let request = AppRequest::ZomeCall(Box::new(
        ZomeCall::try_from_unsigned_zome_call(
            conductor.raw_handle().keystore(),
            ZomeCallUnsigned {
                cell_id: cell.cell_id().clone(),
                zome_name: "sign".into(),
                fn_name: "sign_ephemeral".into(),
                payload: ExternIO::encode(()).unwrap(),
                cap_secret: None,
                provenance: cell.agent_pubkey().clone(),
                nonce,
                expires_at,
            },
        )
        .await
        .unwrap(),
    ));
    if reinstate_mock {
        keystore_control.use_mock();
    }
    client.request(request).await.unwrap()
}

// A test which simulates Keystore errors with a test keystore which is designed
// to fail.
//
// This test was written making the assumption that we could swap out the
// MetaLairClient for each Cell at runtime, but given our current concurrency
// model which puts each Cell in an Arc, this is not possible.
// In order to implement this test, we should probably have the "crude mock
// keystore" listen on a channel which toggles its behavior from always-correct
// to always-failing. However, the problem that this test is testing for does
// not seem to be an issue, therefore I'm not putting the effort into fixing it
// right now.
// @todo fix test by using new InstallApp call
// #[tokio::test(flavor = "multi_thread")]
// async fn test_signing_error_during_genesis_doesnt_bork_interfaces() {
//     observability::test_run().ok();
//     let (keystore, keystore_control) = spawn_real_or_mock_keystore(|_| Err("test error".into()))
//         .await
//         .unwrap();

//     let db_dir = test_db_dir();
//     let config = standard_config();
//     let mut conductor = SweetConductor::new(
//         SweetConductor::handle_from_existing(db_dir.path(), keystore.clone(), &config, &[]).await,
//         db_dir.into(),
//         config,
//     )
//     .await;

//     let (agent1, agent2, agent3) = SweetAgents::three(keystore.clone()).await;

//     let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Sign]).await;

//     let app1 = conductor
//         .setup_app_for_agent("app1", agent1.clone(), &[dna.clone()])
//         .await
//         .unwrap();

//     let app2 = conductor
//         .setup_app_for_agent("app2", agent2.clone(), &[dna.clone()])
//         .await
//         .unwrap();

//     let (cell1,) = app1.into_tuple();
//     let (cell2,) = app2.into_tuple();

//     let app_port = conductor
//         .raw_handle()
//         .add_app_interface(either::Either::Left(0))
//         .await
//         .unwrap();
//     let (mut app_client, _) = websocket_client_by_port(app_port).await.unwrap();
//     let (mut admin_client, _) = conductor.admin_ws_client().await;

//     // Now use the bad keystore to cause a signing error on the next zome call
//     keystore_control.use_mock();

//     let response: AdminResponse = admin_client
//         .request(AdminRequest::InstallApp(Box::new(InstallAppPayload {
//             installed_app_id: "app3".into(),
//             agent_key: agent3.clone(),
//             dnas: vec![InstallAppDnaPayload {
//                 role_name: "whatever".into(),
//                 hash: dna.dna_hash().clone(),
//                 membrane_proof: None,
//             }],
//         })))
//         .await
//         .unwrap();

// assert_matches!(response, AdminResponse::Error(_));
// let response = make_signing_call(&conductor, &mut app_client, &keystore_control, &cell2).await;

//     assert_matches!(response, AppResponse::Error(_));

//     // Go back to the good keystore, see if we can proceed
//     keystore_control.use_real();

// let response = make_signing_call(&conductor, &mut app_client, &keystore_control, &cell2).await;
// assert_matches!(response, AppResponse::ZomeCall(_));

// let response = make_signing_call(&conductor, &mut app_client, &keystore_control, &cell1).await;
// assert_matches!(response, AppResponse::ZomeCall(_));
// }

pub(crate) fn simple_create_entry_zome() -> InlineIntegrityZome {
    let unit_entry_def = EntryDef::from_id("unit");
    InlineIntegrityZome::new_unique(vec![unit_entry_def.clone()], 0).function(
        "create",
        move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        },
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reenable_app() {
    observability::test_run().ok();
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
    assert_eq!(active_apps[0].cell_data.len(), 2);
    assert_matches!(active_apps[0].status, InstalledAppInfoStatus::Running);

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
    assert_eq!(inactive_apps[0].cell_data.len(), 2);
    assert_matches!(
        inactive_apps[0].status,
        InstalledAppInfoStatus::Disabled {
            reason: DisabledAppReason::User
        }
    );

    conductor.enable_app("app".to_string()).await.unwrap();
    conductor
        .raw_handle()
        .reconcile_cell_status_with_app_status()
        .await
        .unwrap();

    let (_, cell) = app.into_tuple();

    // - We can still make a zome call after reactivation
    let _: ActionHash = conductor
        .call_fallible(&cell.zome("zome"), "create", ())
        .await
        .unwrap();

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
async fn test_installation_fails_if_genesis_self_check_is_invalid() {
    observability::test_run().ok();
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
    observability::test_run().ok();
    let unit_entry_def = EntryDef::from_id("unit");
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

    let mut conductor = SweetConductor::from_standard_config().await;
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
    observability::test_run().ok();
    let unit_entry_def = EntryDef::from_id("unit");
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
    observability::test_run().ok();
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
    assert_matches!(get_status().await, InstalledAppInfoStatus::Paused { .. });

    // PAUSED  --start->  RUNNING

    conductor.start_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Running);

    // RUNNING  --disable->  DISABLED

    conductor
        .disable_app("app".to_string(), DisabledAppReason::User)
        .await
        .unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Disabled { .. });

    // DISABLED  --start->  DISABLED

    conductor.start_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Disabled { .. });

    // DISABLED  --pause->  DISABLED

    conductor
        .pause_app("app".to_string(), PausedAppReason::Error("because".into()))
        .await
        .unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Disabled { .. });

    // DISABLED  --enable->  ENABLED

    conductor.enable_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Running);

    // RUNNING  --pause->  PAUSED

    conductor
        .pause_app("app".to_string(), PausedAppReason::Error("because".into()))
        .await
        .unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Paused { .. });

    // PAUSED  --enable->  RUNNING

    conductor.enable_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Running);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "we don't have the ability to share cells across apps yet, but will need a test for that once we do"]
async fn test_app_status_states_multi_app() {
    todo!("write a test similar to the previous one, testing various state transitions, including switching on and off individual Cells");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cell_and_app_status_reconciliation() {
    observability::test_run().ok();
    use AppStatusFx::*;
    use AppStatusKind::*;
    use CellStatus::*;
    let mk_zome = || ("zome", InlineIntegrityZome::new_unique(Vec::new(), 0));
    let dnas = [
        mk_dna(mk_zome()).await.0,
        mk_dna(mk_zome()).await.0,
        mk_dna(mk_zome()).await.0,
    ];
    let app_id = "app".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    conductor.setup_app(&app_id, &dnas).await.unwrap();

    let cell_ids = conductor.list_cell_ids(None);
    let cell1 = &cell_ids[0..1];

    let check = || async {
        (
            AppStatusKind::from(AppStatus::from(
                conductor.list_apps(None).await.unwrap()[0].status.clone(),
            )),
            conductor.list_cell_ids(Some(Joined)).len(),
            conductor.list_cell_ids(Some(PendingJoin)).len(),
        )
    };

    assert_eq!(check().await, (Running, 3, 0));

    // - Simulate a cell failing to join the network
    conductor.update_cell_status(cell1, PendingJoin);
    assert_eq!(check().await, (Running, 2, 1));

    // - Reconciled app state is Paused due to one unjoined Cell
    let delta = conductor
        .reconcile_app_status_with_cell_status(None)
        .await
        .unwrap();
    assert_eq!(delta, SpinDown);
    assert_eq!(check().await, (Paused, 2, 1));

    // - Can start the app again and get all cells joined
    conductor.start_app(app_id.clone()).await.unwrap();
    assert_eq!(check().await, (Running, 3, 0));

    // - Simulate a cell being removed due to error
    conductor.remove_cells(cell1).await;
    assert_eq!(check().await, (Running, 2, 0));

    // - Again, app state should be reconciled to Paused due to missing cell
    let delta = conductor
        .reconcile_app_status_with_cell_status(None)
        .await
        .unwrap();
    assert_eq!(delta, SpinDown);
    assert_eq!(check().await, (Paused, 2, 0));

    // - Disabling the app causes all cells to be removed
    conductor
        .disable_app(app_id.clone(), DisabledAppReason::User)
        .await
        .unwrap();
    assert_eq!(check().await, (Disabled, 0, 0));

    // - Starting a disabled app does nothing
    conductor.start_app(app_id.clone()).await.unwrap();
    assert_eq!(check().await, (Disabled, 0, 0));

    // - ...but enabling one does
    conductor.enable_app(app_id).await.unwrap();
    assert_eq!(check().await, (Running, 3, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_app_status_filters() {
    observability::test_run().ok();
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
    observability::test_run().ok();
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
    let call_tasks = (0..100 as u32).map(|_i| {
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
