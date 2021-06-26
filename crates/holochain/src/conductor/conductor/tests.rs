use super::Conductor;
use super::ConductorState;
use super::*;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::sweettest::*;
use crate::test_utils::fake_valid_dna_file;
use crate::{
    assert_eq_retry_10s, core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult,
};
use ::fixt::prelude::*;
use holochain_conductor_api::InstalledAppInfoStatus;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppRequest, AppResponse, ZomeCall};
use holochain_keystore::crude_mock_keystore::spawn_crude_mock_keystore;
use holochain_state::prelude::*;
use holochain_types::test_utils::fake_cell_id;
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::WebsocketSender;
use kitsune_p2p_types::dependencies::lair_keystore_api::LairError;
use maplit::hashset;
use matches::assert_matches;

#[tokio::test(flavor = "multi_thread")]
async fn can_update_state() {
    let envs = test_environments();
    let dna_store = MockDnaStore::new();
    let keystore = envs.conductor().keystore().clone();
    let holochain_p2p = holochain_p2p::stub_network().await;
    let conductor = Conductor::new(
        envs.conductor(),
        envs.wasm(),
        envs.p2p(),
        envs.p2p_metrics(),
        dna_store,
        keystore,
        envs.tempdir().path().to_path_buf().into(),
        holochain_p2p,
    )
    .await
    .unwrap();
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state, ConductorState::default());

    let cell_id = fake_cell_id(1);
    let installed_cell = InstalledCell::new(cell_id.clone(), "nick".to_string());
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

#[tokio::test(flavor = "multi_thread")]
async fn can_add_clone_cell_to_app() {
    let envs = test_environments();
    let keystore = envs.conductor().keystore().clone();
    let holochain_p2p = holochain_p2p::stub_network().await;

    let agent = fixt!(AgentPubKey);
    let dna = fake_valid_dna_file("");
    let cell_id = CellId::new(dna.dna_hash().to_owned(), agent.clone());

    let dna_store = RealDnaStore::new();

    let mut conductor = Conductor::new(
        envs.conductor(),
        envs.wasm(),
        envs.p2p(),
        envs.p2p_metrics(),
        dna_store,
        keystore,
        envs.tempdir().path().to_path_buf().into(),
        holochain_p2p,
    )
    .await
    .unwrap();

    let installed_cell = InstalledCell::new(cell_id.clone(), "nick".to_string());
    let slot = AppSlot::new(cell_id.clone(), true, 1);
    let app1 = InstalledAppCommon::new_legacy("no clone", vec![installed_cell.clone()]).unwrap();
    let app2 = InstalledAppCommon::new("yes clone", agent, vec![("nick".into(), slot.clone())]);

    conductor.register_phenotype(dna).await.unwrap();
    conductor
        .update_state(move |mut state| {
            state
                .installed_apps_mut()
                .insert(RunningApp::from(app1.clone()).into());
            state
                .installed_apps_mut()
                .insert(RunningApp::from(app2.clone()).into());
            Ok(state)
        })
        .await
        .unwrap();

    matches::assert_matches!(
        conductor
            .add_clone_cell_to_app(&"no clone".to_string(), &"nick".to_string(), ().into())
            .await,
        Err(ConductorError::AppError(AppError::CloneLimitExceeded(0, _)))
    );

    let cloned_cell_id = conductor
        .add_clone_cell_to_app(&"yes clone".to_string(), &"nick".to_string(), ().into())
        .await
        .unwrap();

    let state = conductor.get_state().await.unwrap();
    assert_eq!(
        state
            .running_apps()
            .find(|(id, _)| &id[..] == "yes clone")
            .unwrap()
            .1
            .cloned_cells()
            .cloned()
            .collect::<Vec<CellId>>(),
        vec![cloned_cell_id]
    );
}

/// App can't be installed if another app is already installed under the
/// same InstalledAppId
#[tokio::test(flavor = "multi_thread")]
async fn app_ids_are_unique() {
    let environments = test_environments();
    let dna_store = MockDnaStore::new();
    let holochain_p2p = holochain_p2p::stub_network().await;
    let mut conductor = Conductor::new(
        environments.conductor(),
        environments.wasm(),
        environments.p2p(),
        environments.p2p_metrics(),
        dna_store,
        environments.keystore().clone(),
        environments.tempdir().path().to_path_buf().into(),
        holochain_p2p,
    )
    .await
    .unwrap();

    let cell_id = fake_cell_id(1);
    let installed_cell = InstalledCell::new(cell_id.clone(), "handle".to_string());
    let app = InstalledAppCommon::new_legacy("id".to_string(), vec![installed_cell]).unwrap();

    conductor
        .add_deactivated_app_to_db(app.clone().into())
        .await
        .unwrap();

    assert_matches!(
        conductor.add_deactivated_app_to_db(app.clone().into()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if id == "id".to_string()
    );

    //- it doesn't matter whether the app is active or inactive
    conductor.enable_app_in_db("id".to_string()).await.unwrap();

    assert_matches!(
        conductor.add_deactivated_app_to_db(app.clone().into()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if id == "id".to_string()
    );
}

/// App can't be installed if it contains duplicate CellNicks
#[tokio::test(flavor = "multi_thread")]
async fn cell_nicks_are_unique() {
    let cells = vec![
        InstalledCell::new(fixt!(CellId), "1".into()),
        InstalledCell::new(fixt!(CellId), "1".into()),
        InstalledCell::new(fixt!(CellId), "2".into()),
    ];
    let result = InstalledAppCommon::new_legacy("id", cells.into_iter());
    matches::assert_matches!(
        result,
        Err(AppError::DuplicateSlotIds(_, nicks)) if nicks == vec!["1".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn can_set_fake_state() {
    let envs = test_environments();
    let state = ConductorState::default();
    let conductor = ConductorBuilder::new()
        .fake_state(state.clone())
        .test(&envs)
        .await
        .unwrap();
    assert_eq!(state, conductor.get_state_from_handle().await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn proxy_tls_with_test_keystore() {
    use ghost_actor::GhostControlSender;

    observability::test_run().ok();

    let keystore1 = spawn_test_keystore().await.unwrap();
    let keystore2 = spawn_test_keystore().await.unwrap();

    if let Err(e) = proxy_tls_inner(keystore1.clone(), keystore2.clone()).await {
        panic!("{:#?}", e);
    }

    let _ = keystore1.ghost_actor_shutdown_immediate().await;
    let _ = keystore2.ghost_actor_shutdown_immediate().await;
}

async fn proxy_tls_inner(
    keystore1: KeystoreSender,
    keystore2: KeystoreSender,
) -> anyhow::Result<()> {
    use ghost_actor::GhostControlSender;
    use kitsune_p2p::dependencies::*;
    use kitsune_p2p_proxy::*;
    use kitsune_p2p_types::transport::*;

    let (cert_digest, cert, cert_priv_key) = keystore1.get_or_create_first_tls_cert().await?;

    let tls_config1 = TlsConfig {
        cert,
        cert_priv_key,
        cert_digest,
    };

    let (cert_digest, cert, cert_priv_key) = keystore2.get_or_create_first_tls_cert().await?;

    let tls_config2 = TlsConfig {
        cert,
        cert_priv_key,
        cert_digest,
    };

    let proxy_config =
        ProxyConfig::local_proxy_server(tls_config1, AcceptProxyCallback::reject_all());
    let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
    let (bind1, mut evt1) = spawn_kitsune_proxy_listener(
        proxy_config,
        kitsune_p2p::dependencies::kitsune_p2p_types::config::KitsuneP2pTuningParams::default(),
        bind,
        evt,
    )
    .await?;
    tokio::task::spawn(async move {
        while let Some(evt) = evt1.next().await {
            match evt {
                TransportEvent::IncomingChannel(_, mut write, read) => {
                    println!("YOOTH");
                    let data = read.read_to_end().await;
                    let data = String::from_utf8_lossy(&data);
                    let data = format!("echo: {}", data);
                    write.write_and_close(data.into_bytes()).await?;
                }
            }
        }
        TransportResult::Ok(())
    });
    let url1 = bind1.bound_url().await?;
    println!("{:?}", url1);

    let proxy_config =
        ProxyConfig::local_proxy_server(tls_config2, AcceptProxyCallback::reject_all());
    let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
    let (bind2, _evt2) = spawn_kitsune_proxy_listener(
        proxy_config,
        kitsune_p2p::dependencies::kitsune_p2p_types::config::KitsuneP2pTuningParams::default(),
        bind,
        evt,
    )
    .await?;
    println!("{:?}", bind2.bound_url().await?);

    let (_url, mut write, read) = bind2.create_channel(url1).await?;
    write.write_and_close(b"test".to_vec()).await?;
    let data = read.read_to_end().await;
    let data = String::from_utf8_lossy(&data);
    assert_eq!("echo: test", data);

    let _ = bind1.ghost_actor_shutdown_immediate().await;
    let _ = bind2.ghost_actor_shutdown_immediate().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_running_apps_for_cell_id() {
    observability::test_run().ok();

    let mk_dna = |name| async move {
        let zome = InlineZome::new_unique(Vec::new());
        SweetDnaFile::unique_from_inline_zome(name, zome)
            .await
            .unwrap()
    };

    // Create three unique DNAs
    let (dna1, _) = mk_dna("zome1").await;
    let (dna2, _) = mk_dna("zome2").await;
    let (dna3, _) = mk_dna("zome3").await;

    // Install two apps on the Conductor:
    // Both share a CellId in common, and also include a distinct CellId each.
    let mut conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;
    let app1 = conductor
        .setup_app_for_agent("app1", alice.clone(), &[dna1.clone(), dna2])
        .await
        .unwrap();
    let app2 = conductor
        .setup_app_for_agent("app2", alice.clone(), &[dna1, dna3])
        .await
        .unwrap();

    let (cell1, cell2) = app1.into_tuple();
    let (_, cell3) = app2.into_tuple();

    let list_apps = |conductor: ConductorHandle, cell: SweetCell| async move {
        conductor
            .list_running_apps_for_cell_id(cell.cell_id())
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

async fn mk_dna(name: &str, zome: InlineZome) -> DnaResult<(DnaFile, Zome)> {
    SweetDnaFile::unique_from_inline_zome(name, zome).await
}

/// A function that sets up a SweetApp, used in several tests in this module
async fn common_genesis_test_app(
    custom_zome: InlineZome,
) -> ConductorResult<(SweetConductor, SweetApp)> {
    let hardcoded_zome = InlineZome::new_unique(Vec::new());

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

    // Create one DNA which works, and one which always panics on validation
    let (dna_hardcoded, _) = mk_dna("hardcoded", hardcoded_zome).await?;
    let (dna_custom, _) = mk_dna("custom", custom_zome).await?;

    // Install both DNAs under the same app:
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor
        .setup_app("app", &[dna_hardcoded, dna_custom])
        .await?;
    Ok((conductor, app))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_uninstall_app() {
    observability::test_run().ok();
    let zome = InlineZome::new_unique(Vec::new());

    let (conductor, _) = common_genesis_test_app(zome).await.unwrap();

    // - Ensure that the app is active
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.running_apps().count(), state.stopped_apps().count())
        },
        (1, 0)
    );

    conductor.uninstall_app(&"app".to_string()).await.unwrap();

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
async fn test_setup_cells_idempotency() {
    observability::test_run().ok();
    let zome = InlineZome::new_unique(Vec::new());
    let (conductor, _) = common_genesis_test_app(zome).await.unwrap();

    conductor.inner_handle().setup_cells().await.unwrap();
    conductor.inner_handle().setup_cells().await.unwrap();

    // - Ensure that the app is active
    assert_eq_retry_10s!(conductor.list_running_apps().await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_signing_error_during_genesis() {
    observability::test_run().ok();
    let bad_keystore = spawn_crude_mock_keystore(|| LairError::other("test error"))
        .await
        .unwrap();

    let envs = test_envs_with_keystore(bad_keystore);
    let config = ConductorConfig::default();
    let mut conductor = SweetConductor::new(
        SweetConductor::handle_from_existing(&envs, &config).await,
        envs,
        config,
    )
    .await;

    let (dna, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Sign])
        .await
        .unwrap();

    let result = conductor
        .setup_app_for_agents("app", &[fixt!(AgentPubKey)], &[dna])
        .await;

    // - Assert that we got an error during Genesis. However, this test is
    //   pretty useless. What we really want is to ensure that the system is
    //   resilient when this type of error comes up in a real setting.
    let err = if let Err(err) = result {
        err
    } else {
        panic!("this should have been an error")
    };

    assert_matches!(err, ConductorError::GenesisFailed { errors } if errors.len() == 1);
}

async fn make_signing_call(client: &mut WebsocketSender, cell: &SweetCell) -> AppResponse {
    client
        .request(AppRequest::ZomeCall(Box::new(ZomeCall {
            cell_id: cell.cell_id().clone(),
            zome_name: "sign".into(),
            fn_name: "sign_ephemeral".into(),
            payload: ExternIO::encode(()).unwrap(),
            cap: None,
            provenance: cell.agent_pubkey().clone(),
        })))
        .await
        .unwrap()
}

/// A test which simulates Keystore errors with a test keystore which is designed
/// to fail.
///
/// This test was written making the assumption that we could swap out the
/// KeystoreSender for each Cell at runtime, but given our current concurrency
/// model which puts each Cell in an Arc, this is not possible.
/// In order to implement this test, we should probably have the "crude mock
/// keystore" listen on a channel which toggles its behavior from always-correct
/// to always-failing. However, the problem that this test is testing for does
/// not seem to be an issue, therefore I'm not putting the effort into fixing it
/// right now.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "we need a better mock keystore in order to implement this test"]
#[allow(unreachable_code, unused_variables, unused_mut)]
async fn test_signing_error_during_genesis_doesnt_bork_interfaces() {
    observability::test_run().ok();
    let good_keystore = spawn_test_keystore().await.unwrap();
    let bad_keystore = spawn_crude_mock_keystore(|| LairError::other("test error"))
        .await
        .unwrap();

    let envs = test_envs_with_keystore(good_keystore.clone());
    let config = standard_config();
    let mut conductor = SweetConductor::new(
        SweetConductor::handle_from_existing(&envs, &config).await,
        envs,
        config,
    )
    .await;

    let (agent1, agent2, agent3) = SweetAgents::three(good_keystore.clone()).await;

    let (dna, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Sign])
        .await
        .unwrap();

    let app1 = conductor
        .setup_app_for_agent("app1", agent1.clone(), &[dna.clone()])
        .await
        .unwrap();

    let app2 = conductor
        .setup_app_for_agent("app2", agent2.clone(), &[dna.clone()])
        .await
        .unwrap();

    let (cell1,) = app1.into_tuple();
    let (cell2,) = app2.into_tuple();

    let app_port = conductor.inner_handle().add_app_interface(0).await.unwrap();
    let (mut app_client, _) = websocket_client_by_port(app_port).await.unwrap();
    let (mut admin_client, _) = conductor.admin_ws_client().await;

    // Now use the bad keystore to cause a signing error on the next zome call
    todo!("switch keystore to always-erroring mode");

    let response: AdminResponse = admin_client
        .request(AdminRequest::InstallApp(Box::new(InstallAppPayload {
            installed_app_id: "app3".into(),
            agent_key: agent3.clone(),
            dnas: vec![InstallAppDnaPayload {
                nick: "whatever".into(),
                hash: dna.dna_hash().clone(),
                membrane_proof: None,
            }],
        })))
        .await
        .unwrap();

    // TODO: match the errors more tightly
    assert_matches!(response, AdminResponse::Error(_));
    let response = make_signing_call(&mut app_client, &cell2).await;
    dbg!(&response);

    assert_matches!(response, AppResponse::Error(_));

    // Go back to the good keystore, see if we can proceed
    todo!("switch keystore to always-correct mode");

    let response = make_signing_call(&mut app_client, &cell2).await;
    assert_matches!(response, AppResponse::ZomeCall(_));

    let response = make_signing_call(&mut app_client, &cell1).await;
    assert_matches!(response, AppResponse::ZomeCall(_));

    // conductor
    //     .setup_app_for_agent("app3", agent3, &[dna.clone()])
    //     .await
    //     .unwrap();
}

pub(crate) fn simple_create_entry_zome() -> InlineZome {
    let unit_entry_def = EntryDef::default_with_id("unit");
    InlineZome::new_unique(vec![unit_entry_def.clone()]).callback("create", move |api, ()| {
        let entry_def_id: EntryDefId = unit_entry_def.id.clone();
        let entry = Entry::app(().try_into().unwrap()).unwrap();
        let hash = api.create(EntryWithDefId::new(entry_def_id, entry))?;
        Ok(hash)
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reactivate_app() {
    observability::test_run().ok();
    let zome = simple_create_entry_zome();
    let (conductor, app) = common_genesis_test_app(zome).await.unwrap();

    let all_apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(all_apps.len(), 1);

    let inactive_apps = conductor
        .list_apps(Some(InstalledAppStatusFilter::Disabled))
        .await
        .unwrap();
    let active_apps = conductor
        .list_apps(Some(InstalledAppStatusFilter::Enabled))
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
        .list_apps(Some(InstalledAppStatusFilter::Disabled))
        .await
        .unwrap();
    let active_apps = conductor
        .list_apps(Some(InstalledAppStatusFilter::Enabled))
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
    conductor.inner_handle().setup_cells().await.unwrap();

    let (_, cell) = app.into_tuple();

    // - We can still make a zome call after reactivation
    let _: HeaderHash = conductor
        .call_fallible(&cell.zome("custom"), "create", ())
        .await
        .unwrap();

    // - Ensure that the app is active

    assert_eq_retry_10s!(conductor.list_running_apps().await.unwrap().len(), 1);
    let inactive_apps = conductor
        .list_apps(Some(InstalledAppStatusFilter::Disabled))
        .await
        .unwrap();
    let active_apps = conductor
        .list_apps(Some(InstalledAppStatusFilter::Enabled))
        .await
        .unwrap();
    assert_eq!(active_apps.len(), 1);
    assert_eq!(inactive_apps.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cells_self_deactivate_on_validation_panic() {
    observability::test_run().ok();
    let bad_zome =
        InlineZome::new_unique(Vec::new()).callback("validate", |_api, _data: ValidateData| {
            panic!("intentional panic during validation");
            #[allow(unreachable_code)]
            Ok(ValidateResult::Valid)
        });

    let (conductor, _) = common_genesis_test_app(bad_zome).await.unwrap();

    // - Ensure that the app was deactivated because one Cell panicked during validation
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.running_apps().count(), state.stopped_apps().count())
        },
        (0, 1)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cells_proceed_on_validation_error() {
    observability::test_run().ok();
    let bad_zome =
        InlineZome::new_unique(Vec::new()).callback("validate", |_api, _data: ValidateData| {
            InlineZomeResult::<ValidateResult>::Err(InlineZomeError::TestError(
                "intentional error".into(),
            ))
        });

    let (conductor, _) = common_genesis_test_app(bad_zome).await.unwrap();

    // - Ensure that the app continues to run even with a validation error
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.running_apps().count(), state.stopped_apps().count())
        },
        (1, 0)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_installation_fails_if_genesis_self_check_is_invalid() {
    observability::test_run().ok();
    let bad_zome = InlineZome::new_unique(Vec::new()).callback(
        "genesis_self_check",
        |_api, _data: GenesisSelfCheckData| {
            Ok(GenesisSelfCheckResult::Invalid(
                "intentional invalid result for testing".into(),
            ))
        },
    );

    let err = if let Err(err) = common_genesis_test_app(bad_zome).await {
        err
    } else {
        panic!("this should have been an error")
    };

    assert_matches!(err, ConductorError::GenesisFailed { errors } if errors.len() == 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_bad_entry_validation_after_genesis_returns_zome_call_error() {
    observability::test_run().ok();
    let unit_entry_def = EntryDef::default_with_id("unit");
    let bad_zome = InlineZome::new_unique(vec![unit_entry_def.clone()])
        .callback("validate_create_entry", |_api, _data: ValidateData| {
            Ok(ValidateResult::Invalid(
                "intentional invalid result for testing".into(),
            ))
        })
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = unit_entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(EntryWithDefId::new(entry_def_id, entry))?;
            Ok(hash)
        });

    let (conductor, app) = common_genesis_test_app(bad_zome).await.unwrap();

    let (_, cell_bad) = app.into_tuple();

    let result: ConductorApiResult<HeaderHash> = conductor
        .call_fallible(&cell_bad.zome("custom"), "create", ())
        .await;

    // - The failed validation simply causes the zome call to return an error
    assert_matches!(result, Err(_));

    // - The app is not deactivated
    assert_eq_retry_10s!(
        {
            let state = conductor.get_state_from_handle().await.unwrap();
            (state.running_apps().count(), state.stopped_apps().count())
        },
        (1, 0)
    );
}

// TODO: we need a test with a failure during a validation callback that happens
//       *inline*. It's not enough to have a failing validate_create_entry for
//       instance, because that failure will be returned by the zome call.
//
// NB: currently the pre-genesis and post-genesis handling of panics is the same.
//   If we implement [ B-04188 ], then this test will be made more possible.
//   Otherwise, we have to devise a way to discover whether a panic happened
//   during genesis or not.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "need to figure out how to write this test"]
async fn test_apps_deactivate_on_panic_after_genesis() {
    observability::test_run().ok();
    let unit_entry_def = EntryDef::default_with_id("unit");
    let bad_zome = InlineZome::new_unique(vec![unit_entry_def.clone()])
        // We need a different validation callback that doesn't happen inline
        // so we can cause failure in it. But it must also be after genesis.
        .callback("validate_create_entry", |_api, _data: ValidateData| {
            // Trigger a deserialization error
            let _: Entry = SerializedBytes::try_from(())?.try_into()?;
            Ok(ValidateResult::Valid)
        })
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = unit_entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(EntryWithDefId::new(entry_def_id, entry))?;
            Ok(hash)
        });

    let (conductor, app) = common_genesis_test_app(bad_zome).await.unwrap();

    let (_, cell_bad) = app.into_tuple();

    let _: ConductorApiResult<HeaderHash> = conductor
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
    let (conductor, _) = common_genesis_test_app(zome).await.unwrap();

    let all_apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(all_apps.len(), 1);

    let get_status = || async { conductor.list_apps(None).await.unwrap()[0].status.clone() };

    // RUNNING -pause-> PAUSED

    conductor
        .pause_app("app".to_string(), PausedAppReason::User)
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
        .pause_app("app".to_string(), PausedAppReason::User)
        .await
        .unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Disabled { .. });

    // DISABLED  --enable->  ENABLED

    conductor.enable_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Running);

    // RUNNING  --pause->  PAUSED

    conductor
        .pause_app("app".to_string(), PausedAppReason::User)
        .await
        .unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Paused { .. });

    // PAUSED  --enable->  RUNNING

    conductor.enable_app("app".to_string()).await.unwrap();
    assert_matches!(get_status().await, InstalledAppInfoStatus::Running);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_app_status_filters() {
    observability::test_run().ok();
    let zome = InlineZome::new_unique(Vec::new());
    let dnas = [mk_dna("dna", zome).await.unwrap().0];

    let mut conductor = SweetConductor::from_standard_config().await;

    conductor.setup_app("running", &dnas).await.unwrap();
    conductor.setup_app("paused", &dnas).await.unwrap();
    conductor.setup_app("disabled", &dnas).await.unwrap();

    // put apps in the proper states for testing

    conductor
        .pause_app("paused".to_string(), PausedAppReason::User)
        .await
        .unwrap();

    conductor
        .disable_app("disabled".to_string(), DisabledAppReason::User)
        .await
        .unwrap();

    // Check the counts returned by each filter

    assert_eq!(conductor.list_apps(None).await.unwrap().len(), 3);
    assert_eq!(
        conductor
            .list_apps(Some(InstalledAppStatusFilter::Running))
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        conductor
            .list_apps(Some(InstalledAppStatusFilter::Stopped))
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        conductor
            .list_apps(Some(InstalledAppStatusFilter::Enabled))
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        conductor
            .list_apps(Some(InstalledAppStatusFilter::Disabled))
            .await
            .unwrap()
            .len(),
        1
    );
}
