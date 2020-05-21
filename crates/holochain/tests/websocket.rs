use anyhow::Result;
use assert_cmd::prelude::*;
use futures::Future;
use holochain_2020::conductor::{
    api::{AdminRequest, AdminResponse, AppRequest, AppResponse},
    config::*,
    error::ConductorError,
    Conductor, ConductorHandle,
};
use holochain_types::{
    cell::CellId,
    dna::{DnaFile, Properties},
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    observability,
    prelude::*,
    test_utils::{
        fake_agent_pubkey_1, fake_cap_token, fake_dna_file, fake_dna_zomes, fake_header_hash,
        write_fake_dna_file,
    },
};
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::*;
use holochain_zome_types::*;
use matches::assert_matches;
use std::sync::Arc;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tempdir::TempDir;
use test_wasm_common::TestString;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::stream::StreamExt;
use tracing::*;
use url2::prelude::*;
use uuid::Uuid;

fn spawn_output(holochain: &mut Child) {
    let stdout = holochain.stdout.take();
    let stderr = holochain.stderr.take();
    tokio::task::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                trace!("holochain bin stdout: {}", line);
            }
        }
    });
    tokio::task::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                trace!("holochain bin stderr: {}", line);
            }
        }
    });
}

async fn check_started(holochain: &mut Child) {
    let started = tokio::time::timeout(std::time::Duration::from_secs(1), holochain).await;
    if let Ok(status) = started {
        panic!("Holochain failed to start. status: {:?}", status);
    }
}

fn create_config(port: u16, environment_path: PathBuf) -> ConductorConfig {
    ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
        }]),
        environment_path: environment_path.into(),
        network: None,
        signing_service_uri: None,
        encryption_service_uri: None,
        decryption_service_uri: None,
        dpki: None,
        passphrase_service: PassphraseServiceConfig::Mock {
            passphrase: "password".into(),
        },
    }
}

fn write_config(mut path: PathBuf, config: &ConductorConfig) -> PathBuf {
    path.push("conductor_config.toml");
    std::fs::write(path.clone(), toml::to_string(&config).unwrap()).unwrap();
    path
}

#[instrument(skip(holochain, response))]
async fn check_timeout<T>(
    holochain: &mut Child,
    response: impl Future<Output = Result<T, std::io::Error>>,
    timeout_millis: u64,
) -> T {
    match tokio::time::timeout(std::time::Duration::from_millis(timeout_millis), response).await {
        Ok(response) => response.unwrap(),
        Err(_) => {
            holochain.kill().unwrap();
            error!("Timeout");
            panic!("Timed out on request");
        }
    }
}

async fn admin_port(conductor: &ConductorHandle) -> u16 {
    conductor
        .get_arbitrary_admin_websocket_port()
        .await
        .expect("No admin port open on conductor")
}

async fn websocket_client(
    conductor: &ConductorHandle,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let port = admin_port(conductor).await;
    websocket_client_by_port(port).await
}

async fn websocket_client_by_port(port: u16) -> Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

fn zome_invocation_from_names(
    zome_name: &str,
    fn_name: &str,
    payload: SerializedBytes,
    cell_id: CellId,
) -> ZomeInvocation {
    ZomeInvocation {
        zome_name: zome_name.into(),
        fn_name: fn_name.into(),
        cell_id,
        cap: fake_cap_token(),
        payload: ZomeExternHostInput::new(payload),
        provenance: fake_agent_pubkey_1(),
        as_at: fake_header_hash("fake"),
    }
}

#[tokio::test(threaded_scheduler)]
async fn call_admin() {
    observability::test_run().ok();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    // TODO: B-01453: can we make this port 0 and find out the dynamic port later?
    let port = 9909;

    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(port, environment_path);
    let config_path = write_config(path, &config);

    let cmd = std::process::Command::cargo_bin("holochain-2020").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg("--structured")
        .arg("--config-path")
        .arg(config_path)
        .env("RUST_LOG", "debug")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    spawn_output(&mut holochain);
    check_started(&mut holochain).await;

    let (mut client, _) = websocket_client_by_port(port).await.unwrap();

    let uuid = Uuid::new_v4();
    let dna = fake_dna_file(&uuid.to_string());
    let original_dna_hash = dna.dna_hash().clone();

    // Make properties
    let json = serde_json::json!({
        "test": "example",
        "how_many": 42,
    });
    let properties = Some(json.clone());

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    let request = AdminRequest::InstallDna(fake_dna_path, properties.clone());
    let response = client.request(request);
    let response = check_timeout(&mut holochain, response, 1000).await;
    assert_matches!(response, AdminResponse::DnaInstalled);

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = client.request(request);
    let response = check_timeout(&mut holochain, response, 1000).await;

    let tmp_wasm = dna.code().values().cloned().collect::<Vec<_>>();
    let mut tmp_dna = dna.dna().clone();
    tmp_dna.properties = Properties::new(properties.unwrap()).try_into().unwrap();
    let dna = DnaFile::new(tmp_dna, tmp_wasm).await.unwrap();

    assert_ne!(&original_dna_hash, dna.dna_hash());

    let expects = vec![dna.dna_hash().clone()];
    assert_matches!(response, AdminResponse::ListDnas(a) if a == expects);

    holochain.kill().expect("Failed to kill holochain");
}

async fn start_holochain(config_path: PathBuf) -> Child {
    let cmd = std::process::Command::cargo_bin("holochain-2020").unwrap();
    let mut cmd = Command::from(cmd);
    cmd.arg("--structured")
        .arg("--config-path")
        .arg(config_path)
        .env("RUST_LOG", "trace")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    spawn_output(&mut holochain);
    check_started(&mut holochain).await;
    holochain
}

async fn call_foo_fn(app_port: u16, original_dna_hash: DnaHash, holochain: &mut Child) {
    // Connect to App Interface
    let (mut app_interface, _) = websocket_client_by_port(app_port).await.unwrap();

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct Payload {
        a: u32,
    }
    let payload = Payload { a: 1 };
    let cell_id = CellId::from((original_dna_hash, fake_agent_pubkey_1()));
    let request = Box::new(zome_invocation_from_names(
        "zomey",
        "foo",
        payload.try_into().unwrap(),
        cell_id,
    ));
    let request = AppRequest::ZomeInvocationRequest { request };
    let response = app_interface.request(request);
    let call_response = check_timeout(holochain, response, 2000).await;
    let foo = TestString::from(String::from("foo"));
    let expected = Box::new(ZomeInvocationResponse::ZomeApiFn(
        ZomeExternGuestOutput::new(foo.try_into().unwrap()),
    ));
    trace!(?call_response);
    assert_matches!(call_response, AppResponse::ZomeInvocationResponse{ response } if response == expected);
    app_interface
        .close(1000, "Shutting down".into())
        .await
        .unwrap();
}

async fn attach_app_interface(client: &mut WebsocketSender, holochain: &mut Child) -> u16 {
    let request = AdminRequest::AttachAppInterface { port: None };
    let response = client.request(request);
    let response = check_timeout(holochain, response, 1000).await;
    match response {
        AdminResponse::AppInterfaceAttached { port } => port,
        _ => panic!("Attach app interface failed: {:?}", response),
    }
}

async fn retry_admin_interface(port: u16, mut attempts: usize, delay: Duration) -> WebsocketSender {
    loop {
        match websocket_client_by_port(port).await {
            Ok(c) => return c.0,
            Err(e) => {
                attempts -= 1;
                if attempts == 0 {
                    panic!("Failed to join admin interface");
                }
                warn!(
                    "Failed with {:?} to open admin interface, trying {} more times",
                    e, attempts
                );
                tokio::time::delay_for(delay).await;
            }
        }
    }
}

#[tokio::test(threaded_scheduler)]
async fn call_zome() {
    observability::test_run().ok();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    // TODO: B-01453: can we make this port 0 and find out the dynamic port later?
    let port = 9910;

    let tmp_dir = TempDir::new("conductor_cfg_2").unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(port, environment_path);
    let config_path = write_config(path, &config);

    let mut holochain = start_holochain(config_path.clone()).await;

    let (mut client, _) = websocket_client_by_port(port).await.unwrap();

    let uuid = Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
        vec![("zomey".into(), TestWasm::Foo.into())],
    );
    let original_dna_hash = dna.dna_hash().clone();

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    let request = AdminRequest::InstallDna(fake_dna_path, None);
    let response = client.request(request);
    let response = check_timeout(&mut holochain, response, 3000).await;
    assert_matches!(response, AdminResponse::DnaInstalled);

    // List Dnas
    let request = AdminRequest::ListDnas;
    let response = client.request(request);
    let response = check_timeout(&mut holochain, response, 1000).await;

    let expects = vec![original_dna_hash.clone()];
    assert_matches!(response, AdminResponse::ListDnas(a) if a == expects);

    // Activate cells
    let dna_hashes = vec![(original_dna_hash.clone(), None)];
    let request = AdminRequest::ActivateApp {
        hashes_with_proofs: dna_hashes,
        agent_key: fake_agent_pubkey_1(),
    };
    let response = client.request(request);
    let response = check_timeout(&mut holochain, response, 1000).await;
    assert_matches!(response, AdminResponse::AppsActivated);

    // Attach App Interface
    let app_port = attach_app_interface(&mut client, &mut holochain).await;

    // Call Zome
    call_foo_fn(app_port, original_dna_hash.clone(), &mut holochain).await;

    client.close(1000, "Shutting down".into()).await.unwrap();
    // Shutdown holochain
    holochain.kill().expect("Failed to kill holochain");
    std::mem::drop(client);

    // Call zome after resart
    let mut holochain = start_holochain(config_path).await;
    let mut client = retry_admin_interface(port, 3, Duration::from_millis(100)).await;

    // Attach App Interface
    let app_port = attach_app_interface(&mut client, &mut holochain).await;

    // Call Zome again
    call_foo_fn(app_port, original_dna_hash, &mut holochain).await;

    // Shutdown holochain
    holochain.kill().expect("Failed to kill holochain");
}

#[tokio::test(threaded_scheduler)]
async fn conductor_admin_interface_runs_from_config() -> Result<()> {
    observability::test_run().ok();
    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path);
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let (mut client, _) = websocket_client(&conductor_handle).await?;

    let (fake_dna_path, _tmpdir) = write_fake_dna_file(fake_dna_file("")).await.unwrap();
    let request = AdminRequest::InstallDna(fake_dna_path, None);
    let response = client.request(request).await;
    assert_matches!(response, Ok(AdminResponse::DnaInstalled));
    conductor_handle.shutdown().await;

    Ok(())
}

#[tokio::test(threaded_scheduler)]
async fn conductor_admin_interface_ends_with_shutdown() -> Result<()> {
    observability::test_run().ok();

    info!("creating config");
    let tmp_dir = TempDir::new("conductor_cfg").unwrap();
    let environment_path = tmp_dir.path().to_path_buf();
    let config = create_config(0, environment_path);
    let conductor_handle = Conductor::builder().config(config).build().await?;
    let port = admin_port(&conductor_handle).await;
    info!("building conductor");
    let (mut client, rx): (WebsocketSender, WebsocketReceiver) = websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig {
            default_request_timeout_s: 1,
            ..Default::default()
        }),
    )
    .await?;

    info!("client connect");

    conductor_handle.shutdown().await;

    info!("shutdown");

    assert_matches!(
        conductor_handle.check_running().await,
        Err(ConductorError::ShuttingDown)
    );

    let incoming: Vec<_> = rx.collect().await;
    assert_eq!(incoming.len(), 1);
    assert_matches!(incoming[0], WebsocketMessage::Close(_));

    info!("About to make failing request");

    let (fake_dna, _tmpdir) = write_fake_dna_file(fake_dna_file("")).await.unwrap();
    let request = AdminRequest::InstallDna(fake_dna, None);

    // send a request after the conductor has shutdown
    let response: Result<Result<AdminResponse, _>, tokio::time::Elapsed> =
        tokio::time::timeout(Duration::from_secs(1), client.request(request)).await;

    // request should have errored since the conductor shut down,
    // but should not have timed out (which would be an `Err(Err(_))`)
    assert_matches!(response, Ok(Err(_)));

    Ok(())
}
