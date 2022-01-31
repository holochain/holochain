use ::fixt::prelude::*;
use anyhow::Result;
use futures::future;
use hdk::prelude::RemoteSignal;
use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetConductorBatch;
use holochain::sweettest::SweetDnaFile;
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
    test_utils::{fake_agent_pubkey_1, fake_dna_zomes, write_fake_dna_file},
};
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::*;
use matches::assert_matches;
use observability;
use std::sync::Arc;
use std::time::Duration;
use tempdir::TempDir;
use tokio_stream::StreamExt;
use tracing::*;
use url2::prelude::*;

use test_utils::*;

pub mod test_utils;

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn call_admin() {
    observability::test_run().ok();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    // MAYBE: B-01453: can we make this port 0 and find out the dynamic port later?
    let port = 9909;

    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(port, environment_path);
    let config_path = write_config(path, &config);

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    );

    let _holochain = start_holochain(config_path.clone()).await;

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
        "role_id".into(),
        6000,
    )
    .await;

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = client.request(request);
    let response = check_timeout(response, 6000).await;

    let tmp_wasm = dna.code().values().cloned().collect::<Vec<_>>();
    let mut tmp_dna = dna.dna_def().clone();
    tmp_dna.properties = properties.try_into().unwrap();
    let dna = holochain_types::dna::DnaFile::new(tmp_dna, tmp_wasm)
        .await
        .unwrap();

    assert_ne!(&original_dna_hash, dna.dna_hash());

    let expects = vec![dna.dna_hash().clone()];
    assert_matches!(response, AdminResponse::DnasListed(a) if a == expects);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn call_zome() {
    observability::test_run().ok();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    // MAYBE: B-01453: can we make this port 0 and find out the dynamic port later?
    let admin_port = 9910;
    let app_port = 9913;

    let tmp_dir = TempDir::new("conductor_cfg_2").unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(admin_port, environment_path);
    let config_path = write_config(path, &config);

    let holochain = start_holochain(config_path.clone()).await;

    let (mut client, _) = websocket_client_by_port(admin_port).await.unwrap();
    let (_, receiver2) = websocket_client_by_port(admin_port).await.unwrap();

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    );
    let original_dna_hash = dna.dna_hash().clone();

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    let _dna_hash = register_and_install_dna(
        &mut client,
        original_dna_hash.clone(),
        fake_agent_pubkey_1(),
        fake_dna_path,
        None,
        "".into(),
        6000,
    )
    .await;

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = client.request(request);
    let response = check_timeout(response, 3000).await;

    let expects = vec![original_dna_hash.clone()];
    assert_matches!(response, AdminResponse::DnasListed(a) if a == expects);

    // Activate cells
    let request = AdminRequest::EnableApp {
        installed_app_id: "test".to_string(),
    };
    let response = client.request(request);
    let response = check_timeout(response, 3000).await;
    assert_matches!(response, AdminResponse::AppEnabled { .. });

    // Attach App Interface
    let app_port_rcvd = attach_app_interface(&mut client, Some(app_port)).await;
    assert_eq!(app_port, app_port_rcvd);

    // Call Zome
    tracing::info!("Calling zome");
    call_foo_fn(app_port, original_dna_hash.clone()).await;

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
    std::mem::drop(client);

    // Call zome after restart
    tracing::info!("Restarting conductor");
    let _holochain = start_holochain(config_path).await;

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    // Call Zome again on the existing app interface port
    tracing::info!("Calling zome again");
    call_foo_fn(app_port, original_dna_hash).await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn remote_signals() -> anyhow::Result<()> {
    observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 2;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    // MAYBE: write helper for agents across conductors
    let all_agents: Vec<HoloHash<hash_type::Agent>> =
        future::join_all(conductors.iter().map(|c| SweetAgents::one(c.keystore()))).await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .unwrap()
        .0;

    let apps = conductors
        .setup_app_for_zipped_agents("app", &all_agents, &[dna_file])
        .await
        .unwrap();

    conductors.exchange_peer_info().await;

    let cells = apps.cells_flattened();

    let mut rxs = Vec::new();
    for h in conductors.iter().map(|c| c) {
        rxs.push(h.signal_broadcaster().await.subscribe_separately())
    }
    let rxs = rxs.into_iter().flatten().collect::<Vec<_>>();

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

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let signal = AppSignal::new(signal);
    for mut rx in rxs {
        let r = rx.try_recv();
        // Each handle should recv a signal
        assert_matches!(r, Ok(Signal::App(_, a)) if a == signal);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn emit_signals() {
    observability::test_run().ok();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    // MAYBE: B-01453: can we make this port 0 and find out the dynamic port later?
    let admin_port = 9911;

    let tmp_dir = TempDir::new("conductor_cfg_emit_signals").unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(admin_port, environment_path);
    let config_path = write_config(path, &config);

    let _holochain = start_holochain(config_path.clone()).await;

    let (mut admin_tx, _) = websocket_client_by_port(admin_port).await.unwrap();

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![(TestWasm::EmitSignal.into(), TestWasm::EmitSignal.into())],
    );
    let orig_dna_hash = dna.dna_hash().clone();
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna).await.unwrap();
    // Install Dna
    let agent_key = fake_agent_pubkey_1();

    let dna_hash = register_and_install_dna(
        &mut admin_tx,
        orig_dna_hash,
        fake_agent_pubkey_1(),
        fake_dna_path,
        None,
        "".into(),
        6000,
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

    // Attach App Interface
    let app_port = attach_app_interface(&mut admin_tx, None).await;

    ///////////////////////////////////////////////////////
    // Emit signals (the real test!)

    let (mut app_tx_1, app_rx_1) = websocket_client_by_port(app_port).await.unwrap();
    let (_, app_rx_2) = websocket_client_by_port(app_port).await.unwrap();

    call_zome_fn(
        &mut app_tx_1,
        cell_id.clone(),
        TestWasm::EmitSignal,
        "emit".into(),
        (),
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
        Signal::App(cell_id, AppSignal::new(ExternIO::encode(()).unwrap())),
        Signal::try_from(sig1.clone()).unwrap(),
    );
    assert_eq!(sig1, sig2);

    ///////////////////////////////////////////////////////
}

#[tokio::test(flavor = "multi_thread")]
async fn conductor_admin_interface_runs_from_config() -> Result<()> {
    observability::test_run().ok();
    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path);
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let (mut client, _) = websocket_client(&conductor_handle).await?;

    let dna = fake_dna_zomes(
        "".into(),
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    );
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna).await.unwrap();
    let register_payload = RegisterDnaPayload {
        uid: None,
        properties: None,
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
    observability::test_run().ok();

    info!("creating config");
    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path);
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
    assert_matches!(response, Ok(Ok(AdminResponse::AppInterfacesListed(interfaces))) if interfaces.len() == 0);

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
    observability::test_run().ok();

    info!("creating config");
    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path);
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

    let dna = fake_dna_zomes(
        "".into(),
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    );
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna).await.unwrap();
    let register_payload = RegisterDnaPayload {
        uid: None,
        properties: None,
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
async fn too_many_open() {
    observability::test_run().ok();

    info!("creating config");
    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path);
    let conductor_handle = Conductor::builder().config(config).build().await.unwrap();
    let port = admin_port(&conductor_handle).await;
    info!("building conductor");
    for _i in 0..1000 {
        holochain_websocket::connect(
            url2!("ws://127.0.0.1:{}", port),
            Arc::new(WebsocketConfig {
                default_request_timeout_s: 1,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
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

    observability::test_run().ok();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    // MAYBE: B-01453: can we make this port 0 and find out the dynamic port later?
    let admin_port = 9912;
    // let app_port = 9914;

    let tmp_dir = TempDir::new("conductor_cfg_concurrent_install_dna").unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(admin_port, environment_path);
    let config_path = write_config(path, &config);

    let _holochain = start_holochain(config_path.clone()).await;

    let (client, _) = websocket_client_by_port(admin_port).await.unwrap();

    let before = std::time::Instant::now();

    let install_tasks_stream = futures::stream::iter((0..NUM_DNA).into_iter().map(|i| {
        let zomes = vec![(TestWasm::Foo.into(), TestWasm::Foo.into())];
        let mut client = client.clone();
        tokio::spawn(async move {
            let name = format!("fake_dna_{}", i);

            // Install Dna
            let dna = fake_dna_zomes_named(&uuid::Uuid::new_v4().to_string(), &name, zomes.clone());
            let original_dna_hash = dna.dna_hash().clone();
            let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
            let agent_key = generate_agent_pubkey(&mut client, REQ_TIMEOUT_MS).await;
            println!("[{}] Agent pub key generated", i);

            let dna_hash = register_and_install_dna_named(
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

            println!(
                "[{}] installed dna with hash {} and name {}",
                i, dna_hash, name
            );
        })
    }))
    .buffer_unordered(NUM_CONCURRENT_INSTALLS.into());

    let install_tasks = futures::StreamExt::collect::<Vec<_>>(install_tasks_stream);

    for r in install_tasks.await {
        r.unwrap();
    }

    println!(
        "installed {} dna in {:?}",
        NUM_CONCURRENT_INSTALLS,
        before.elapsed()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn full_state_dump_cursor_works() {
    observability::test_run().ok();

    let mut conductor = SweetConductor::from_standard_config().await;

    let agent = SweetAgents::one(conductor.keystore()).await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .unwrap()
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
