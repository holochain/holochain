use assert_cmd::prelude::*;
use holochain_conductor_api::{AdminRequest, AppRequest};
use holochain_conductor_api::{AdminResponse, AppResponse};
use holochain_websocket::{
    self as ws, WebsocketConfig, WebsocketReceiver, WebsocketResult, WebsocketSender,
};
use matches::assert_matches;
use once_cell::sync::Lazy;
use portpicker::pick_unused_port;
use proc_ctl::{PortQuery, ProcQuery, ProtocolPort};
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use url2::url2;
use which::which;

const WEBSOCKET_TIMEOUT: Duration = Duration::from_secs(3);

static HC_BUILT_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_path.push("../hc/Cargo.toml");

    let out = escargot::CargoBuild::new()
        .bin("hc")
        .current_target()
        .current_release()
        .manifest_path(manifest_path)
        // Not defined on CI
        .target_dir(PathBuf::from(
            option_env!("CARGO_TARGET_DIR").unwrap_or("./target"),
        ))
        .run()
        .unwrap();

    out.path().to_path_buf()
});

static HOLOCHAIN_BUILT_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_path.push("../holochain/Cargo.toml");

    let out = escargot::CargoBuild::new()
        .bin("holochain")
        .current_target()
        .current_release()
        .manifest_path(manifest_path)
        .target_dir(PathBuf::from(
            option_env!("CARGO_TARGET_DIR").unwrap_or("./target"),
        ))
        .run()
        .unwrap();

    out.path().to_path_buf()
});

async fn new_websocket_client_for_port(
    port: u16,
) -> anyhow::Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(ws::connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

async fn get_app_info(port: u16) {
    tracing::debug!(calling_app_interface = ?port);
    let (mut app_tx, _) = new_websocket_client_for_port(port).await.expect(&format!(
        "Failed to connect to conductor on port [{}]",
        port
    ));
    let request = AppRequest::AppInfo {
        installed_app_id: "Stub".to_string(),
    };
    let response = app_tx.request(request);
    let r: AppResponse = check_timeout(response).await;
    assert_matches!(r, AppResponse::AppInfo(None));
}

async fn check_timeout<T>(response: impl Future<Output = Result<T, ws::WebsocketError>>) -> T {
    match tokio::time::timeout(WEBSOCKET_TIMEOUT, response).await {
        Ok(response) => response.expect("Calling websocket failed"),
        Err(_) => {
            panic!("Timed out on request after {:?}", WEBSOCKET_TIMEOUT);
        }
    }
}

async fn package_fixture_if_not_packaged() {
    if PathBuf::from("tests/fixtures/my-app/my-fixture-app.happ").exists() {
        return;
    }

    get_hc_command()
        .arg("dna")
        .arg("pack")
        .arg("tests/fixtures/my-app/dna")
        .stdout(Stdio::null())
        .status()
        .await
        .expect("Failed to pack DNA");

    get_hc_command()
        .arg("app")
        .arg("pack")
        .arg("tests/fixtures/my-app")
        .stdout(Stdio::null())
        .status()
        .await
        .expect("Failed to pack hApp");
}

async fn clean_sandboxes() {
    get_sandbox_command()
        .arg("clean")
        .stdout(Stdio::null())
        .status()
        .await
        .unwrap();
}

/// Generates a new sandbox with a single app deployed and tries to get app info
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_connect() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run().ok();
    let app_port: u16 = pick_unused_port().expect("No ports free");
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg(format!("--run={}", app_port))
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true);

    let hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    let hc_pid = hc_admin.id().unwrap();

    let mut child_stdin = hc_admin.stdin.unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    let ports = get_holochain_ports_from_hc_process(hc_pid).await;
    let (_, app_ports) = partition_ports(ports.as_slice()).await;

    // - Make a call to list app info to the port
    get_app_info(*app_ports.first().expect("No app ports found")).await;
}

/// Generates a new sandbox with a single app deployed and tries to list DNA
#[tokio::test(flavor = "multi_thread")]
async fn generate_sandbox_and_call_list_dna() {
    clean_sandboxes().await;
    package_fixture_if_not_packaged().await;

    holochain_trace::test_run().ok();
    let app_port: u16 = pick_unused_port().expect("No ports free");
    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("generate")
        .arg("--in-process-lair")
        .arg(format!("--run={}", app_port))
        .arg("tests/fixtures/my-app/")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true);

    let hc_admin = cmd.spawn().expect("Failed to spawn holochain");
    let hc_pid = hc_admin.id().unwrap();
    let mut child_stdin = hc_admin.stdin.unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    let ports = get_holochain_ports_from_hc_process(hc_pid).await;
    let (admin_ports, app_ports) = partition_ports(ports.as_slice()).await;

    // - Make a call to list app info to the port
    get_app_info(*app_ports.first().unwrap()).await;

    let mut cmd = get_sandbox_command();
    cmd.env("RUST_BACKTRACE", "1")
        .arg(format!("-f={}", admin_ports.first().unwrap()))
        .arg(format!(
            "--holochain-path={}",
            get_holochain_bin_path().to_str().unwrap()
        ))
        .arg("--piped")
        .arg("call")
        .arg("list-dnas")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true);
    let mut hc_call = cmd.spawn().expect("Failed to spawn holochain");
    let mut child_stdin = hc_call.stdin.take().unwrap();
    child_stdin.write_all(b"test-phrase\n").await.unwrap();
    drop(child_stdin);

    let exit_code = hc_call.wait().await.unwrap();
    assert!(exit_code.success());
}

fn get_hc_command() -> Command {
    Command::new(match which("hc") {
        Ok(p) => p,
        Err(_) => HC_BUILT_PATH.clone(),
    })
}

fn get_holochain_bin_path() -> PathBuf {
    match which("holochain") {
        Ok(p) => p,
        Err(_) => HOLOCHAIN_BUILT_PATH.clone(),
    }
}

fn get_sandbox_command() -> Command {
    match which("hc-sandbox") {
        Ok(p) => Command::new(p),
        Err(_) => Command::from(std::process::Command::cargo_bin("hc-sandbox").unwrap()),
    }
}

async fn get_holochain_ports_from_hc_process(sandbox_pid: u32) -> Vec<u16> {
    // Have to retry because holochain gets launched more than once by the sandbox to generate config
    for _ in 0..10 {
        let holochain_pid = get_holochain_pid_from_sandbox(sandbox_pid).await;
        let ports = get_holochain_bound_ports(holochain_pid, 0).await;

        if ports.len() >= 2 {
            return ports;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    panic!("Could not find ports for Holochain");
}

async fn get_holochain_pid_from_sandbox(hc_pid: u32) -> u32 {
    ProcQuery::new()
        .process_id(hc_pid)
        .expect_min_num_children(1)
        .children_with_retry(Duration::from_millis(1000), 10)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|p| {
            if p.name.as_str() == "holochain" {
                Some(p.pid)
            } else {
                None
            }
        })
        .next()
        .unwrap()
}

async fn get_holochain_bound_ports(holochain_pid: u32, minimum_ports: usize) -> Vec<u16> {
    PortQuery::new()
        .process_id(holochain_pid)
        .tcp_only()
        .expect_min_num_ports(minimum_ports)
        .execute_with_retry(Duration::from_millis(1000), 30)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|p| {
            if let ProtocolPort::Tcp(p) = p {
                Some(p)
            } else {
                None
            }
        })
        .collect()
}

async fn is_admin_port(port: u16) -> bool {
    let (mut app_tx, _) = new_websocket_client_for_port(port).await.expect(&format!(
        "Failed to connect to conductor on port [{}]",
        port
    ));
    let request = AdminRequest::ListDnas;
    let response: Result<WebsocketResult<AdminResponse>, _> =
        tokio::time::timeout(WEBSOCKET_TIMEOUT, app_tx.request(request)).await;

    response
        .map(|v| match v {
            Ok(AdminResponse::Error(_)) | Err(_) => false,
            Ok(_) => true,
        })
        .unwrap()
}

async fn partition_ports(candidate_ports: &[u16]) -> (Vec<u16>, Vec<u16>) {
    let mut admin_ports = vec![];
    let mut app_ports = vec![];
    for &port in candidate_ports {
        if is_admin_port(port).await {
            admin_ports.push(port);
        } else {
            app_ports.push(port);
        }
    }

    (admin_ports, app_ports)
}
