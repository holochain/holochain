use ::fixt::prelude::*;
use anyhow::Result;
use futures::future;
use hdk::prelude::RemoteSignal;
use holochain::conductor::interface::websocket::MAX_CONNECTIONS;
use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetConductorBatch;
use holochain::sweettest::SweetDnaFile;
use holochain::sweettest::{SweetAgents, SweetConductorConfig};
use holochain::{
    conductor::{
        api::{AdminRequest, AdminResponse},
        error::ConductorError,
        Conductor,
    },
    fixt::*,
};

use holochain_types::{
    prelude::*,
    test_utils::{fake_dna_zomes, write_fake_dna_file},
};
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::*;
use matches::assert_matches;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio_stream::StreamExt;
use tracing::*;
use url2::prelude::*;

use crate::test_utils::*;

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "glacial_tests")]
async fn call_admin() {
    holochain_trace::test_run().ok();
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

    let (mut client, _) = websocket_client_by_port(port).await.unwrap();

    let original_dna_hash = dna.dna_hash().clone();

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

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();

    let orig_dna_hash = dna.dna_hash().clone();
    register_and_install_dna(
        &mut client,
        orig_dna_hash,
        fake_agent_pubkey_1(),
        fake_dna_path,
        Some(properties.clone()),
        "role_name".into(),
        10000,
    )
    .await;

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = client.request(request);
    let response = check_timeout(response, 10000).await;

    let tmp_wasm = dna.code().values().cloned().collect::<Vec<_>>();
    let mut tmp_dna = dna.dna_def().clone();
    tmp_dna.modifiers.properties = properties.try_into().unwrap();
    let dna = holochain_types::dna::DnaFile::new(tmp_dna, tmp_wasm).await;

    assert_ne!(&original_dna_hash, dna.dna_hash());

    let expects = vec![dna.dna_hash().clone()];
    assert_matches!(response, AdminResponse::DnasListed(a) if a == expects);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "glacial_tests")]
async fn call_zome() {
    holochain_trace::test_run().ok();
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

    let (mut admin_tx, _) = websocket_client_by_port(admin_port).await.unwrap();
    let (_, receiver2) = websocket_client_by_port(admin_port).await.unwrap();

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    );
    let original_dna_hash = dna.dna_hash().clone();

    let agent_key = fake_agent_pubkey_1();

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    let dna_hash = register_and_install_dna(
        &mut admin_tx,
        original_dna_hash.clone(),
        agent_key.clone(),
        fake_dna_path,
        None,
        "".into(),
        10000,
    )
    .await;
    let cell_id = CellId::new(dna_hash.clone(), agent_key.clone());

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await;

    let expects = vec![original_dna_hash.clone()];
    assert_matches!(response, AdminResponse::DnasListed(a) if a == expects);

    // Activate cells
    let request = AdminRequest::EnableApp {
        installed_app_id: "test".to_string(),
    };
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await;
    assert_matches!(response, AdminResponse::AppEnabled { .. });

    // Generate signing key pair
    let mut rng = rand_dalek::thread_rng();
    let signing_keypair = ed25519_dalek::Keypair::generate(&mut rng);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.public.as_bytes().to_vec());

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
    .await;

    // Attach App Interface
    let app_port = attach_app_interface(&mut admin_tx, None).await;

    let (mut app_tx, _) = websocket_client_by_port(app_port).await.unwrap();

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
    assert!(Box::pin(receiver2.timeout(Duration::from_millis(500)))
        .next()
        .await
        .unwrap()
        .is_err());

    // Shutdown holochain
    std::mem::drop(holochain);
    std::mem::drop(admin_tx);

    // Call zome after restart
    tracing::info!("Restarting conductor");
    let (_holochain, admin_port) = start_holochain(config_path).await;
    let admin_port = admin_port.await.unwrap();

    let (mut admin_tx, _) = websocket_client_by_port(admin_port).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let request = AdminRequest::ListAppInterfaces;
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await;
    let app_port = match response {
        AdminResponse::AppInterfacesListed(ports) => *ports.first().unwrap(),
        _ => panic!("Unexpected response"),
    };

    let (mut app_tx, _) = websocket_client_by_port(app_port).await.unwrap();

    // Call Zome again on the existing app interface port
    tracing::info!("Calling zome again");
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
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn remote_signals() -> anyhow::Result<()> {
    holochain_trace::test_run().ok();
    const NUM_CONDUCTORS: usize = 2;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    // MAYBE: write helper for agents across conductors
    let all_agents: Vec<HoloHash<hash_type::Agent>> =
        future::join_all(conductors.iter().map(|c| SweetAgents::one(c.keystore()))).await;

    // Check that there are no duplicate agents
    assert_eq!(
        all_agents.len(),
        all_agents
            .clone()
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    );

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let apps = conductors
        .setup_app_for_zipped_agents("app", &all_agents, &[dna_file])
        .await
        .unwrap();

    conductors.exchange_peer_info().await;

    let cells = apps.cells_flattened();

    let mut rxs = Vec::new();
    for h in conductors.iter() {
        rxs.extend(h.signal_broadcaster().subscribe_separately())
    }

    let signal = fixt!(ExternIo);

    let _: () = conductors[0]
        .call(
            &cells[0].zome(TestWasm::EmitSignal),
            "signal_others",
            RemoteSignal {
                signal: signal.clone(),
                agents: all_agents,
            },
        )
        .await;

    tokio::time::timeout(Duration::from_secs(60), async move {
        let signal = AppSignal::new(signal);
        for mut rx in rxs {
            let r = rx.recv().await;
            // Each handle should recv a signal
            assert_matches!(r, Ok(Signal::App{signal: a,..}) if a == signal);
        }
    })
    .await
    .unwrap();

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn emit_signals() {
    holochain_trace::test_run().ok();
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

    let (mut admin_tx, _) = websocket_client_by_port(admin_port).await.unwrap();

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::EmitSignal.into(), TestWasm::EmitSignal.into())],
    );
    let orig_dna_hash = dna.dna_hash().clone();
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna).await.unwrap();

    let agent_key = fake_agent_pubkey_1();

    // Install Dna
    let dna_hash = register_and_install_dna(
        &mut admin_tx,
        orig_dna_hash,
        agent_key.clone(),
        fake_dna_path,
        None,
        "".into(),
        10000,
    )
    .await;
    let cell_id = CellId::new(dna_hash.clone(), agent_key.clone());

    // Activate cells
    let request = AdminRequest::EnableApp {
        installed_app_id: "test".to_string(),
    };
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await;
    assert_matches!(response, AdminResponse::AppEnabled { .. });

    // Generate signing key pair
    let mut rng = rand_dalek::thread_rng();
    let signing_keypair = ed25519_dalek::Keypair::generate(&mut rng);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.public.as_bytes().to_vec());

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
    .await;

    // Attach App Interface
    let app_port = attach_app_interface(&mut admin_tx, None).await;

    ///////////////////////////////////////////////////////
    // Emit signals (the real test!)

    let (mut app_tx_1, app_rx_1) = websocket_client_by_port(app_port).await.unwrap();
    let (_, app_rx_2) = websocket_client_by_port(app_port).await.unwrap();

    call_zome_fn(
        &mut app_tx_1,
        cell_id.clone(),
        &signing_keypair,
        cap_secret,
        zome_name.clone(),
        fn_name,
        &(),
    )
    .await;

    let (sig1, msg1) = Box::pin(app_rx_1.timeout(Duration::from_secs(1)))
        .next()
        .await
        .unwrap()
        .unwrap();
    assert!(!msg1.is_request());

    let (sig2, msg2) = Box::pin(app_rx_2.timeout(Duration::from_secs(1)))
        .next()
        .await
        .unwrap()
        .unwrap();
    assert!(!msg2.is_request());

    assert_eq!(
        Signal::App {
            cell_id,
            zome_name,
            signal: AppSignal::new(ExternIO::encode(()).unwrap()),
        },
        Signal::try_from(sig1.clone()).unwrap(),
    );
    assert_eq!(sig1, sig2);

    ///////////////////////////////////////////////////////
}

#[tokio::test(flavor = "multi_thread")]
async fn conductor_admin_interface_runs_from_config() -> Result<()> {
    holochain_trace::test_run().ok();
    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let (mut client, _) = websocket_client(&conductor_handle).await?;

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
    holochain_trace::test_run().ok();

    info!("creating config");
    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let port = admin_port(&conductor_handle).await;
    info!("building conductor");
    let (mut client, mut _rx): (WebsocketSender, WebsocketReceiver) = holochain_websocket::connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig {
            default_request_timeout_s: 1,
            ..Default::default()
        }),
    )
    .await?;

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
    holochain_trace::test_run().ok();

    info!("creating config");
    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let port = admin_port(&conductor_handle).await;
    info!("building conductor");
    let (mut client, mut rx): (WebsocketSender, WebsocketReceiver) = holochain_websocket::connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig {
            default_request_timeout_s: 1,
            ..Default::default()
        }),
    )
    .await?;

    info!("client connect");

    conductor_handle.shutdown();

    info!("shutdown");

    assert_matches!(
        conductor_handle.check_running(),
        Err(ConductorError::ShuttingDown)
    );

    assert!(rx.next().await.is_none());

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
    // but should not have timed out (which would be an `Err(Err(_))`)
    assert_matches!(response, Ok(Err(WebsocketError::Shutdown)));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn connection_limit_is_respected() {
    holochain_trace::test_run().ok();

    let tmp_dir = TempDir::new().unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path.into());
    let conductor_handle = Conductor::builder().config(config).build().await.unwrap();
    let port = admin_port(&conductor_handle).await;

    let url = url2!("ws://127.0.0.1:{}", port);
    let cfg = Arc::new(WebsocketConfig::default());

    // Retain handles so that the test can control when to disconnect clients
    let mut handles = Vec::new();

    // The first `MAX_CONNECTIONS` connections should succeed
    for _ in 0..MAX_CONNECTIONS {
        let (mut sender, _) = connect(url.clone(), cfg.clone()).await.unwrap();
        let _: AdminResponse = sender
            .request(AdminRequest::ListDnas)
            .await
            .expect("Admin request should succeed because there are enough available connections");
        handles.push(sender);
    }

    // Try lots of failed connections to make sure the limit is respected
    for _ in 0..2 * MAX_CONNECTIONS {
        let (mut sender, _) = connect(url.clone(), cfg.clone()).await.unwrap();

        // Getting a sender back isn't enough to know that the connection succeeded because the other side takes a moment to shutdown, try sending to be sure
        sender
            .request::<AdminRequest, AdminResponse>(AdminRequest::ListDnas)
            .await
            .expect_err("Should be no available connection slots");
    }

    // Disconnect all the clients
    handles.clear();

    // Should now be possible to connect new clients
    for _ in 0..MAX_CONNECTIONS {
        let (mut sender, _) = connect(url.clone(), cfg.clone()).await.unwrap();
        let _: AdminResponse = sender
            .request(AdminRequest::ListDnas)
            .await
            .expect("Admin request should succeed because there are enough available connections");
        handles.push(sender);
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

    holochain_trace::test_run().ok();
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

    let (client, _) = websocket_client_by_port(admin_port).await.unwrap();

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
            let original_dna_hash = dna.dna_hash().clone();
            let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
            let agent_key = generate_agent_pubkey(&mut client, REQ_TIMEOUT_MS).await;
            // println!("[{}] Agent pub key generated", i);

            let _dna_hash = register_and_install_dna_named(
                &mut client,
                original_dna_hash.clone(),
                agent_key,
                fake_dna_path.clone(),
                None,
                name.clone(),
                name.clone(),
                REQ_TIMEOUT_MS,
            )
            .await;

            // println!(
            //     "[{}] installed dna with hash {} and name {}",
            //     i, _dna_hash, name
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
    holochain_trace::test_run().ok();

    let mut batch =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;

    let dna_file = SweetDnaFile::unique_empty().await;

    let _ = batch.setup_app("app", &[dna_file]).await.unwrap();
    batch.exchange_peer_info().await;

    let (mut client, _) = batch.get(0).unwrap().admin_ws_client().await;

    #[cfg(not(feature = "tx5"))]
    const EXPECT: &str = "tx2-quic";
    #[cfg(feature = "tx5")]
    const EXPECT: &str = "go-pion";

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
    holochain_trace::test_run().ok();

    let mut conductor = SweetConductor::from_standard_config().await;

    let agent = SweetAgents::one(conductor.keystore()).await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna_file])
        .await
        .unwrap();

    let cell_id = app.into_cells()[0].cell_id().clone();

    let (mut client, _) = conductor.admin_ws_client().await;

    let full_state = dump_full_state(&mut client, cell_id.clone(), None).await;

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
    .await;

    let integrated_ops_count = full_state.integration_dump.integrated.len();
    let validation_limbo_ops_count = full_state.integration_dump.validation_limbo.len();
    let integration_limbo_ops_count = full_state.integration_dump.integration_limbo.len();

    let new_all_dht_ops_count =
        integrated_ops_count + validation_limbo_ops_count + integration_limbo_ops_count;

    assert_eq!(1, new_all_dht_ops_count);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
// NOTE: This is a full integration test that
// actually runs the holochain binary
async fn repro_network_join_failure() {
    holochain_trace::test_run().ok();
    static REQ_TIMEOUT_MS: u64 = 15000;

    let alice_dir = TempDir::new().unwrap();
    let bob_dir = TempDir::new().unwrap();
    let alice_path = alice_dir.path().to_path_buf();
    let bob_path = bob_dir.path().to_path_buf();
    let alice_root = alice_path.clone();
    let bob_root = bob_path.clone();

    let (signal_addr, _abort_handle) = kitsune_p2p::test_util::start_signal_srv();
    let mut kconfig = kitsune_p2p_types::config::KitsuneP2pConfig::default();
    kconfig.transport_pool = vec![kitsune_p2p_types::config::TransportConfig::WebRTC {
        signal_url: format!("ws://{:?}", signal_addr),
    }];

    let mut alice_config = create_config(0, alice_root.into());
    let mut bob_config = create_config(0, bob_root.into());
    alice_config.network = kconfig.clone();
    bob_config.network = kconfig;

    let zomes = vec![(TestWasm::Foo.into(), TestWasm::Foo.into())];
    let name = format!("fake_dna");

    // Install Dna
    let dna = holochain_types::test_utils::fake_dna_zomes_named(
        &uuid::Uuid::new_v4().to_string(),
        &name,
        zomes.clone(),
    );
    let original_dna_hash = dna.dna_hash().clone();
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();

    let alice_config_path = write_config(alice_path.clone(), &alice_config);

    let (alice_process, alice_port) = start_holochain(alice_config_path.clone()).await;
    let (bob_process, bob_port) = start_holochain(write_config(bob_path, &bob_config)).await;

    let alice_port = alice_port.await.unwrap();
    let bob_port = bob_port.await.unwrap();

    let (mut alice_ws, _) = websocket_client_by_port(alice_port).await.unwrap();
    let (mut bob_ws, _) = websocket_client_by_port(bob_port).await.unwrap();

    let alice_agent = generate_agent_pubkey(&mut alice_ws, REQ_TIMEOUT_MS).await;
    let bob_agent = generate_agent_pubkey(&mut bob_ws, REQ_TIMEOUT_MS).await;
    // println!("[{}] Agent pub key generated", i);

    let alice_dna_hash = register_and_install_dna_named(
        &mut alice_ws,
        original_dna_hash.clone(),
        alice_agent,
        fake_dna_path.clone(),
        None,
        name.clone(),
        name.clone(),
        REQ_TIMEOUT_MS,
    )
    .await;

    let bob_dna_hash = register_and_install_dna_named(
        &mut bob_ws,
        original_dna_hash.clone(),
        bob_agent,
        fake_dna_path.clone(),
        None,
        name.clone(),
        name.clone(),
        REQ_TIMEOUT_MS,
    )
    .await;

    assert_eq!(alice_dna_hash, bob_dna_hash);

    let alice_res: AdminResponse = alice_ws
        .request(AdminRequest::EnableApp {
            installed_app_id: name.clone(),
        })
        .await
        .unwrap();
    dbg!(alice_res);
    let bob_res: AdminResponse = bob_ws
        .request(AdminRequest::EnableApp {
            installed_app_id: name.clone(),
        })
        .await
        .unwrap();
    dbg!(bob_res);

    if let AdminResponse::NetworkMetricsDumped(metrics) = alice_ws
        .request(AdminRequest::DumpNetworkMetrics { dna_hash: None })
        .await
        .unwrap()
    {
        dbg!(metrics);
    } else {
        unreachable!();
    }

    let before = std::time::Instant::now();

    drop(alice_process);
    // drop(bob_process);

    let (alice_process, alice_port) = start_holochain(alice_config_path).await;

    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

    let alice_port = alice_port.await.unwrap();

    let (mut alice_ws, _) = websocket_client_by_port(alice_port).await.unwrap();

    if let AdminResponse::AppEnabled { app, errors: _ } = alice_ws
        .request(AdminRequest::EnableApp {
            installed_app_id: name,
        })
        .await
        .unwrap()
    {
        assert_eq!(app.status, holochain_conductor_api::AppInfoStatus::Running);
    } else {
        unreachable!();
    }
}

/*

/// Test that network join doesn't hang up just because some recently-seen
/// peers are not available
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn test_network_join_real() -> anyhow::Result<()> {
    let t = tempfile::TempDir::new().unwrap();
    let path = t.path().join("config.yaml");
    let config = SweetConductorConfig::rendezvous(true);

    let (signal_addr, abort_handle) = start_signal_srv();
    let mut kconfig = kitsune_p2p_types::config::KitsuneP2pConfig::default();
    kconfig.transport_pool = vec![kitsune_p2p_types::config::TransportConfig::WebRTC {
        signal_url: format!("ws://{:?}", signal_addr),
    }];
    let mut config = ConductorConfig::default();
    config.network = kconfig;

    std::fs::write(&path, serde_yaml::to_string(&config)?)?;

    let (alice, alice_port) = crate::test_utils::start_holochain(path.clone()).await;
    let (bob, bob_port) = crate::test_utils::start_holochain(path.clone()).await;

    let alice_port = alice_port.await?;
    let bob_port = bob_port.await?;

    dbg!(alice_port, bob_port);

    Ok(())
}

*/
