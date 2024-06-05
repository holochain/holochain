use ::fixt::prelude::*;
use anyhow::Result;
use hdk::prelude::RemoteSignal;
use holochain::conductor::interface::websocket::MAX_CONNECTIONS;
use holochain::sweettest::SweetConductorBatch;
use holochain::sweettest::SweetConductorConfig;
use holochain::sweettest::SweetDnaFile;
use holochain::sweettest::{authenticate_app_ws_client, SweetConductor, WsPollRecv};
use holochain::{
    conductor::{
        api::{AdminRequest, AdminResponse, AppResponse},
        error::ConductorError,
        Conductor,
    },
    fixt::*,
};
use std::net::{Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use either::Either;
use holochain_conductor_api::{AdminInterfaceConfig, AppRequest, InterfaceDriver};
use holochain_types::websocket::AllowedOrigins;
use holochain_types::{
    prelude::*,
    test_utils::{fake_dna_zomes, write_fake_dna_file},
};
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::*;
use matches::assert_matches;
use rand::rngs::OsRng;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tracing::*;

use crate::tests::test_utils::*;

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn call_admin() {
    holochain_trace::test_run();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    let port = 0;

    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(port, environment_path.into());
    let config_path = write_config(path, &config);

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    );

    let (_holochain, port) = start_holochain(config_path.clone()).await;
    let port = port.await.unwrap();

    let (mut client, rx) = websocket_client_by_port(port).await.unwrap();
    let _rx = WsPollRecv::new::<AdminResponse>(rx);

    // Make properties
    let properties = holochain_zome_types::properties::YamlProperties::new(
        serde_yaml::from_str(
            r#"
test: "example"
how_many: 42
    "#,
        )
        .unwrap(),
    );

    let original_dna_hash = dna.dna_hash().clone();

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();

    let installed_cell_id = register_and_install_dna(
        &mut client,
        fake_dna_path,
        Some(properties.clone()),
        "role_name".into(),
        10000,
    )
    .await
    .unwrap();

    let installed_dna_hash = installed_cell_id.dna_hash().clone();

    assert_ne!(installed_dna_hash, original_dna_hash);

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = client.request(request);
    let response = check_timeout(response, 10000).await.unwrap();

    assert_matches!(response, AdminResponse::DnasListed(a) if a.contains(&installed_dna_hash));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn call_zome() {
    holochain_trace::test_run();

    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    let admin_port = 0;

    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(admin_port, environment_path.into());
    let config_path = write_config(path, &config);

    let (holochain, admin_port) = start_holochain(config_path.clone()).await;
    let admin_port = admin_port.await.unwrap();

    let (mut admin_tx, admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(admin_rx);
    let (_, mut receiver2) = websocket_client_by_port(admin_port).await.unwrap();

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    );

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    let cell_id = register_and_install_dna(&mut admin_tx, fake_dna_path, None, "".into(), 10000)
        .await
        .unwrap();
    let installed_dna_hash = cell_id.dna_hash().clone();

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = admin_tx.request(request);
    let response = check_timeout(response, 15000).await.unwrap();

    assert_matches!(response, AdminResponse::DnasListed(a) if a.contains(&installed_dna_hash));

    // Activate cells
    let request = AdminRequest::EnableApp {
        installed_app_id: "test".to_string(),
    };
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await.unwrap();
    assert_matches!(response, AdminResponse::AppEnabled { .. });

    // Generate signing key pair
    let mut rng = OsRng;
    let signing_keypair = ed25519_dalek::SigningKey::generate(&mut rng);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());

    // Grant zome call capability for agent
    let zome_name = TestWasm::Foo.coordinator_zome_name();
    let fn_name = FunctionName("foo".into());
    let cap_secret = grant_zome_call_capability(
        &mut admin_tx,
        &cell_id,
        zome_name.clone(),
        fn_name.clone(),
        signing_key,
    )
    .await
    .unwrap();

    // Attach App Interface
    let app_port = attach_app_interface(&mut admin_tx, None).await;

    let (mut app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);
    authenticate_app_ws_client(app_tx.clone(), admin_port, "test".to_string()).await;

    // Call Zome
    tracing::info!("Calling zome");
    call_zome_fn(
        &mut app_tx,
        cell_id.clone(),
        &signing_keypair,
        cap_secret,
        zome_name.clone(),
        fn_name.clone(),
        &(),
    )
    .await;

    // Ensure that the other client does not receive any messages, i.e. that
    // responses are not broadcast to all connected clients, only the one
    // that made the request.
    // Err means the timeout elapsed
    assert!(tokio::time::timeout(
        Duration::from_millis(500),
        receiver2.recv::<AdminResponse>(),
    )
    .await
    .is_err());

    // Shutdown holochain
    drop(holochain);
    drop(admin_tx);

    // Call zome after restart
    info!("Restarting conductor");
    let (_holochain, admin_port) = start_holochain(config_path).await;
    let admin_port = admin_port.await.unwrap();

    let (admin_tx, admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(admin_rx);

    tokio::time::sleep(Duration::from_millis(1000)).await;

    let request = AdminRequest::ListAppInterfaces;
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await.unwrap();
    let app_port = match response {
        AdminResponse::AppInterfacesListed(ports) => ports.first().map(|i| i.port).unwrap(),
        _ => panic!("Unexpected response"),
    };

    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);
    authenticate_app_ws_client(app_tx.clone(), admin_port, "test".to_string()).await;

    // Call Zome again on the existing app interface port
    tracing::info!("Calling zome again");
    call_zome_fn(
        &app_tx,
        cell_id.clone(),
        &signing_keypair,
        cap_secret,
        zome_name.clone(),
        fn_name.clone(),
        &(),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn remote_signals() -> anyhow::Result<()> {
    use std::collections::HashSet;

    holochain_trace::test_run();
    const NUM_CONDUCTORS: usize = 2;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let all_agents: HashSet<_> = apps
        .cells_flattened()
        .into_iter()
        .map(|c| c.agent_pubkey().clone())
        .collect();

    assert_eq!(all_agents.len(), NUM_CONDUCTORS);

    conductors.exchange_peer_info().await;

    let cells = apps.cells_flattened();

    let mut rxs = Vec::new();
    for h in conductors.iter() {
        rxs.push(h.subscribe_to_app_signals("app".to_string()))
    }

    let signal = fixt!(ExternIo);

    let _: () = conductors[0]
        .call(
            &cells[0].zome(TestWasm::EmitSignal),
            "signal_others",
            RemoteSignal {
                signal: signal.clone(),
                agents: all_agents.into_iter().collect(),
            },
        )
        .await;

    tokio::time::timeout(Duration::from_secs(60), async move {
        let signal = AppSignal::new(signal);
        for mut rx in rxs {
            let r = rx.recv().await;
            // Each handle should recv a signal
            match r {
                Ok(Signal::App { signal: r, .. }) => {
                    assert_eq!(r, signal);
                }
                oth => panic!("unexpected: {oth:?}"),
            }
        }
    })
    .await
    .unwrap();

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn emit_signals() {
    holochain_trace::test_run();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    let admin_port = 0;

    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(admin_port, environment_path.into());
    let config_path = write_config(path, &config);

    let (_holochain, admin_port) = start_holochain(config_path.clone()).await;
    let admin_port = admin_port.await.unwrap();

    let (mut admin_tx, admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(admin_rx);

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::EmitSignal.into(), TestWasm::EmitSignal.into())],
    );
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna).await.unwrap();

    // Install Dna
    let cell_id = register_and_install_dna(&mut admin_tx, fake_dna_path, None, "".into(), 10000)
        .await
        .unwrap();

    // Activate cells
    let request = AdminRequest::EnableApp {
        installed_app_id: "test".to_string(),
    };
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await.unwrap();
    assert_matches!(response, AdminResponse::AppEnabled { .. });

    // Generate signing key pair
    let mut rng = OsRng;
    let signing_keypair = ed25519_dalek::SigningKey::generate(&mut rng);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());

    // Grant zome call capability for agent
    let zome_name = TestWasm::EmitSignal.coordinator_zome_name();
    let fn_name = FunctionName("emit".into());
    let cap_secret = grant_zome_call_capability(
        &mut admin_tx,
        &cell_id,
        zome_name.clone(),
        fn_name.clone(),
        signing_key,
    )
    .await
    .unwrap();

    // Attach App Interface
    let app_port = attach_app_interface(&mut admin_tx, None).await;

    ///////////////////////////////////////////////////////
    // Emit signals (the real test!)

    let (app_tx_1, mut app_rx_1) = websocket_client_by_port(app_port).await.unwrap();
    let (sig1_send, sig1_recv) = tokio::sync::oneshot::channel();
    let mut sig1_send = Some(sig1_send);
    let sig1_task = tokio::task::spawn(async move {
        loop {
            match app_rx_1.recv::<AppResponse>().await {
                Ok(ReceiveMessage::Signal(sig1)) => {
                    if let Some(sig1_send) = sig1_send.take() {
                        let _ = sig1_send.send(sig1);
                    }
                }
                oth => panic!("unexpected: {oth:?}"),
            }
        }
    });
    authenticate_app_ws_client(app_tx_1.clone(), admin_port, "test".to_string()).await;

    let (app_tx_2, mut app_rx_2) = websocket_client_by_port(app_port).await.unwrap();
    let (sig2_send, sig2_recv) = tokio::sync::oneshot::channel();
    let mut sig2_send = Some(sig2_send);
    let sig2_task = tokio::task::spawn(async move {
        loop {
            match app_rx_2.recv::<AppResponse>().await {
                Ok(ReceiveMessage::Signal(sig2)) => {
                    if let Some(sig2_send) = sig2_send.take() {
                        let _ = sig2_send.send(sig2);
                    }
                }
                oth => panic!("unexpected: {oth:?}"),
            }
        }
    });
    authenticate_app_ws_client(app_tx_2.clone(), admin_port, "test".to_string()).await;

    call_zome_fn(
        &app_tx_1,
        cell_id.clone(),
        &signing_keypair,
        cap_secret,
        zome_name.clone(),
        fn_name,
        &(),
    )
    .await;

    let sig1 = Signal::try_from_vec(sig1_recv.await.unwrap()).unwrap();
    let sig2 = Signal::try_from_vec(sig2_recv.await.unwrap()).unwrap();
    sig1_task.abort();
    sig2_task.abort();

    assert_eq!(
        Signal::App {
            cell_id,
            zome_name,
            signal: AppSignal::new(ExternIO::encode(()).unwrap()),
        },
        sig1,
    );
    assert_eq!(sig1, sig2);

    ///////////////////////////////////////////////////////
}

#[tokio::test(flavor = "multi_thread")]
async fn conductor_admin_interface_runs_from_config() -> Result<()> {
    holochain_trace::test_run();
    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let (client, rx) = websocket_client(&conductor_handle).await?;
    let _rx = WsPollRecv::new::<AdminResponse>(rx);

    let dna = fake_dna_zomes("", vec![(TestWasm::Foo.into(), TestWasm::Foo.into())]);
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna).await.unwrap();
    let register_payload = RegisterDnaPayload {
        modifiers: DnaModifiersOpt::none(),
        source: DnaSource::Path(fake_dna_path),
    };
    let request = AdminRequest::RegisterDna(Box::new(register_payload));
    let response = client.request(request).await.unwrap();
    assert_matches!(response, AdminResponse::DnaRegistered(_));

    conductor_handle.shutdown();

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn list_app_interfaces_succeeds() -> Result<()> {
    holochain_trace::test_run();

    info!("creating config");
    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let port = admin_port(&conductor_handle).await;
    info!("building conductor");
    let mut ws_config = WebsocketConfig::CLIENT_DEFAULT;
    ws_config.default_request_timeout = Duration::from_secs(1);
    let (client, rx): (WebsocketSender, WebsocketReceiver) = connect(
        Arc::new(ws_config),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
        ),
    )
    .await?;
    let _rx = WsPollRecv::new::<AdminResponse>(rx);

    let request = AdminRequest::ListAppInterfaces;

    // Request the list of app interfaces that the conductor has attached
    let response: Result<Result<AdminResponse, _>, tokio::time::error::Elapsed> =
        tokio::time::timeout(Duration::from_secs(1), client.request(request)).await;

    // There should be no app interfaces listed
    assert_matches!(response, Ok(Ok(AdminResponse::AppInterfacesListed(interfaces))) if interfaces.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn conductor_admin_interface_ends_with_shutdown() -> Result<()> {
    if let Err(e) = conductor_admin_interface_ends_with_shutdown_inner().await {
        panic!("{:#?}", e);
    }
    Ok(())
}

async fn conductor_admin_interface_ends_with_shutdown_inner() -> Result<()> {
    holochain_trace::test_run();

    info!("creating config");
    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let port = admin_port(&conductor_handle).await;
    info!("building conductor");
    let mut ws_config = WebsocketConfig::CLIENT_DEFAULT;
    ws_config.default_request_timeout = Duration::from_secs(1);
    let (client, mut rx): (WebsocketSender, WebsocketReceiver) = holochain_websocket::connect(
        Arc::new(ws_config),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
        ),
    )
    .await?;

    info!("client connect");

    conductor_handle.shutdown();

    info!("shutdown");

    assert_matches!(
        conductor_handle.check_running(),
        Err(ConductorError::ShuttingDown)
    );

    assert!(tokio::time::timeout(
        std::time::Duration::from_secs(7),
        rx.recv::<AdminResponse>(),
    )
    .await
    .unwrap()
    .is_err());

    info!("About to make failing request");

    let dna = fake_dna_zomes("", vec![(TestWasm::Foo.into(), TestWasm::Foo.into())]);
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna).await.unwrap();
    let register_payload = RegisterDnaPayload {
        modifiers: DnaModifiersOpt::none(),
        source: DnaSource::Path(fake_dna_path),
    };
    let request = AdminRequest::RegisterDna(Box::new(register_payload));

    // send a request after the conductor has shutdown
    // let response: Result<Result<AdminResponse, _>, tokio::time::Elapsed> =
    //     tokio::time::timeout(Duration::from_secs(1), client.request(request)).await;
    let response: Result<Result<AdminResponse, _>, tokio::time::error::Elapsed> =
        tokio::time::timeout(Duration::from_secs(1), client.request(request)).await;

    // request should have encountered an error since the conductor shut down,
    // but should not have timed out (which would be an `Err(_)`)
    assert_matches!(response, Ok(Err(_)));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn connection_limit_is_respected() {
    holochain_trace::test_run();

    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await.unwrap();
    let port = admin_port(&conductor_handle).await;

    let addr = format!("localhost:{port}")
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();
    let cfg = Arc::new(WebsocketConfig::CLIENT_DEFAULT);

    // Retain handles so that the test can control when to disconnect clients
    let mut handles = Vec::new();

    tracing::warn!("OPEN FIRST CONNECTION");
    // The first `MAX_CONNECTIONS` connections should succeed
    for count in 0..MAX_CONNECTIONS {
        let (sender, rx) = connect(cfg.clone(), addr).await.unwrap();
        let rx = WsPollRecv::new::<AdminResponse>(rx);
        let _: AdminResponse = sender
            .request(AdminRequest::ListDnas)
            .await
            .map_err(|e| Error::other(format!("Admin request should succeed because there are enough available connections: {count}: {e:?}")))
            .unwrap();
        handles.push((sender, rx));
    }

    // Try lots of failed connections to make sure the limit is respected
    for _ in 0..2 * MAX_CONNECTIONS {
        let (sender, rx) = connect(cfg.clone(), addr).await.unwrap();
        let _rx = WsPollRecv::new::<AdminResponse>(rx);

        // Getting a sender back isn't enough to know that the connection succeeded because the other side takes a moment to shutdown, try sending to be sure
        sender
            .request::<AdminRequest, AdminResponse>(AdminRequest::ListDnas)
            .await
            .expect_err("Should be no available connection slots");
    }

    // Disconnect all the clients
    handles.clear();

    // Should now be possible to connect new clients
    for count in 0..MAX_CONNECTIONS {
        let (sender, rx) = connect(cfg.clone(), addr).await.unwrap();
        let rx = WsPollRecv::new::<AdminResponse>(rx);
        let _: AdminResponse = sender
            .request(AdminRequest::ListDnas)
            .await
            .map_err(|e| Error::other(format!("Admin request should succeed because there are enough available connections: {count}: {e:?}")))
            .unwrap();
        handles.push((sender, rx));
    }

    conductor_handle.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
// TODO: duplicate/rewrite this to also test happ bundles in addition to dna
async fn concurrent_install_dna() {
    use futures::StreamExt;

    static NUM_DNA: u8 = 50;
    static NUM_CONCURRENT_INSTALLS: u8 = 10;
    static REQ_TIMEOUT_MS: u64 = 15000;

    holochain_trace::test_run();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    let admin_port = 0;

    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().to_path_buf();
    let data_root_path = path.clone();
    let config = create_config(admin_port, data_root_path.into());
    let config_path = write_config(path, &config);

    let (_holochain, admin_port) = start_holochain(config_path.clone()).await;
    let admin_port = admin_port.await.unwrap();

    let (client, rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _rx = WsPollRecv::new::<AdminResponse>(rx);

    // let before = std::time::Instant::now();

    let install_tasks_stream = futures::stream::iter((0..NUM_DNA).map(|i| {
        let zomes = vec![(TestWasm::Foo.into(), TestWasm::Foo.into())];
        let mut client = client.clone();
        tokio::spawn(async move {
            let name = format!("fake_dna_{}", i);

            // Install Dna
            let dna = holochain_types::test_utils::fake_dna_zomes_named(
                &uuid::Uuid::new_v4().to_string(),
                &name,
                zomes.clone(),
            );
            let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();

            let _cell_id = register_and_install_dna_named(
                &mut client,
                fake_dna_path.clone(),
                None,
                name.clone(),
                name.clone(),
                REQ_TIMEOUT_MS,
            )
            .await;

            // println!(
            //     "[{}] installed app with cell id {} and name {}",
            //     i, _cell_id, name
            // );
        })
    }))
    .buffer_unordered(NUM_CONCURRENT_INSTALLS.into());

    let install_tasks = futures::StreamExt::collect::<Vec<_>>(install_tasks_stream);

    for r in install_tasks.await {
        r.unwrap();
    }

    // println!(
    //     "installed {} dna in {:?}",
    //     NUM_DNA,
    //     before.elapsed()
    // );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn network_stats() {
    holochain_trace::test_run();

    let mut batch =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;

    let dna_file = SweetDnaFile::unique_empty().await;

    let _ = batch.setup_app("app", &[dna_file]).await.unwrap();
    batch.exchange_peer_info().await;

    let (client, _rx) = batch
        .get(0)
        .unwrap()
        .admin_ws_client::<AdminResponse>()
        .await;

    #[cfg(feature = "tx5")]
    const EXPECT: &str = "backendGoPion";

    let req = AdminRequest::DumpNetworkStats;
    let res: AdminResponse = client.request(req).await.unwrap();
    match res {
        AdminResponse::NetworkStatsDumped(json) => {
            println!("{json}");

            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            let backend = parsed.as_object().unwrap().get("backend").unwrap();
            assert_eq!(EXPECT, backend);
        }
        _ => panic!("unexpected"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn full_state_dump_cursor_works() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let app = conductor.setup_app("app", &[dna_file]).await.unwrap();

    let cell_id = app.into_cells()[0].cell_id().clone();

    let (mut client, _rx) = conductor.admin_ws_client::<AppResponse>().await;

    let full_state = dump_full_state(&mut client, cell_id.clone(), None)
        .await
        .unwrap();

    let integrated_ops_count = full_state.integration_dump.integrated.len();
    let validation_limbo_ops_count = full_state.integration_dump.validation_limbo.len();
    let integration_limbo_ops_count = full_state.integration_dump.integration_limbo.len();

    let all_dhts_ops_count =
        integrated_ops_count + validation_limbo_ops_count + integration_limbo_ops_count;
    assert_eq!(7, all_dhts_ops_count);

    // We are assuming we have at least one DhtOp in the Cell
    let full_state = dump_full_state(
        &mut client,
        cell_id,
        Some(full_state.integration_dump.dht_ops_cursor - 1),
    )
    .await
    .unwrap();

    let integrated_ops_count = full_state.integration_dump.integrated.len();
    let validation_limbo_ops_count = full_state.integration_dump.validation_limbo.len();
    let integration_limbo_ops_count = full_state.integration_dump.integration_limbo.len();

    let new_all_dht_ops_count =
        integrated_ops_count + validation_limbo_ops_count + integration_limbo_ops_count;

    assert_eq!(1, new_all_dht_ops_count);
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_allowed_origins() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    let ports = conductor
        .clone()
        .add_admin_interfaces(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket {
                port: 0,
                allowed_origins: "http://localhost:3000".to_string().into(),
            },
        }])
        .await
        .unwrap();

    let port = *ports.first().unwrap();
    assert!(connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap()
        )
    )
    .await
    .is_err());

    let port = *ports.first().unwrap();
    let (client, rx) = connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
        )
        .try_set_header("origin", "http://localhost:3000")
        .unwrap(),
    )
    .await
    .unwrap();

    let _rx = WsPollRecv::new::<AdminResponse>(rx);

    let request = AdminRequest::ListAppInterfaces;
    let _: AdminResponse = client.request(request).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn holochain_websockets_listen_on_ipv4_and_ipv6() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();

    //
    // Connect to the admin interface on ipv4 and ipv6 localhost
    //

    let (ipv4_admin_sender, rx) = connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new((Ipv4Addr::LOCALHOST, admin_port).into()),
    )
    .await
    .unwrap();
    let _rx4 = WsPollRecv::new::<AdminResponse>(rx);

    let response: AdminResponse = ipv4_admin_sender
        .request(AdminRequest::ListCellIds)
        .await
        .unwrap();
    match response {
        AdminResponse::CellIdsListed(_) => (),
        _ => panic!("unexpected response"),
    }

    let (ipv6_admin_sender, rx) = connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new((Ipv6Addr::LOCALHOST, admin_port).into()),
    )
    .await
    .unwrap();
    let _rx6 = WsPollRecv::new::<AdminResponse>(rx);

    let response: AdminResponse = ipv6_admin_sender
        .request(AdminRequest::ListCellIds)
        .await
        .unwrap();
    match response {
        AdminResponse::CellIdsListed(_) => (),
        _ => panic!("unexpected response"),
    }

    //
    // Do the same for an app interface
    //

    let app_port = conductor
        .clone()
        .add_app_interface(Either::Left(0), AllowedOrigins::Any, None)
        .await
        .unwrap();

    let (ipv4_app_sender, rx) = connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new((Ipv4Addr::LOCALHOST, app_port).into()),
    )
    .await
    .unwrap();
    let _rx4 = WsPollRecv::new::<AppResponse>(rx);
    authenticate_app_ws_client(ipv4_app_sender.clone(), admin_port, "".to_string()).await;

    let response: AppResponse = ipv4_app_sender.request(AppRequest::AppInfo).await.unwrap();
    match response {
        AppResponse::AppInfo(_) => (),
        _ => panic!("unexpected response"),
    }

    let (ipv6_app_sender, rx) = connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new((Ipv6Addr::LOCALHOST, app_port).into()),
    )
    .await
    .unwrap();
    let _rx6 = WsPollRecv::new::<AppResponse>(rx);
    authenticate_app_ws_client(ipv6_app_sender.clone(), admin_port, "".to_string()).await;

    let response: AppResponse = ipv6_app_sender.request(AppRequest::AppInfo).await.unwrap();
    match response {
        AppResponse::AppInfo(_) => (),
        _ => panic!("unexpected response"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn emit_signal_after_app_connection_closed() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    // Install an app to emit signals from
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;
    let installed_app_id: InstalledAppId = "app".into();
    let app = conductor
        .setup_app(&installed_app_id, &[dna_file])
        .await
        .unwrap();
    let cells = app.into_cells();
    let cell = cells.first().unwrap();

    // Connect to the app interface
    let port = conductor
        .clone()
        .add_app_interface(Either::Left(0), AllowedOrigins::Any, None)
        .await
        .expect("Couldn't create app interface");
    let (tx, mut rx) = websocket_client_by_port(port).await.unwrap();

    authenticate_app_ws_client(
        tx.clone(),
        conductor
            .get_arbitrary_admin_websocket_port()
            .expect("No admin ports on this conductor"),
        installed_app_id.clone(),
    )
    .await;

    // Emit a signal
    let _: () = conductor
        .call(&cell.zome(TestWasm::EmitSignal), "emit", ())
        .await;

    // That should be received because the app interface is connected
    let received = rx.recv::<AppResponse>().await.unwrap();
    assert_matches!(received, ReceiveMessage::Signal(_));

    // Drop the app interface connection
    drop(tx);
    drop(rx);

    // Emit another signal
    let _: () = conductor
        .call(&cell.zome(TestWasm::EmitSignal), "emit", ())
        .await;

    // That should not be received because the app interface is disconnected
    // TODO assert that the tasks for this connection were shutdown and removed by this point.
    //      Can't currently do that with TaskMotel which I think is the right thing to query here.
}
