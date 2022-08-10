//! Helpers for running the conductor.
use std::path::Path;
use std::{path::PathBuf, process::Stdio};

use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::calls::attach_app_interface;
use crate::calls::AddAppWs;
use crate::config::*;
use crate::ports::random_admin_port;
use crate::ports::set_admin_port;
use crate::CmdRunner;

/// Run a conductor and wait for it to finish.
/// Use [`run_async`] to run in the background.
/// Requires the holochain binary is available
/// on the `holochain_path`.
/// Uses the sandbox provided by the `sandbox_path`.
/// Adds an app interface in the `app_ports`.
/// Can optionally force the admin port used. Otherwise
/// the port in the config will be used if it's free or
/// a random free port will be chosen.
pub async fn run(
    holochain_path: &Path,
    sandbox_path: PathBuf,
    app_ports: Vec<u16>,
    force_admin_port: Option<u16>,
) -> anyhow::Result<()> {
    let (port, mut holochain, mut lair) =
        run_async(holochain_path, sandbox_path.clone(), force_admin_port).await?;
    msg!("Running conductor on admin port {}", port);
    for app_port in app_ports {
        msg!("Attaching app port {}", app_port);
        let mut cmd = CmdRunner::try_new(port).await?;
        let port = attach_app_interface(
            &mut cmd,
            AddAppWs {
                port: Some(app_port),
            },
        )
        .await?;
        msg!("App port attached at {}", port);
    }
    crate::save::lock_live(std::env::current_dir()?, &sandbox_path, port).await?;
    msg!("Connected successfully to a running holochain");
    let e = format!("Failed to run holochain at {}", sandbox_path.display());

    holochain.wait().await.expect(&e);
    let _ = lair.kill().await;
    lair.wait().await.expect("Failed to wait on lair-keystore");

    Ok(())
}

/// Run a conductor in the background.
/// Requires the holochain binary is available
/// on the `holochain_path`.
/// Uses the sandbox provided by the `sandbox_path`.
/// Can optionally force the admin port used. Otherwise
/// the port in the config will be used if it's free or
/// a random free port will be chosen.
pub async fn run_async(
    holochain_path: &Path,
    sandbox_path: PathBuf,
    force_admin_port: Option<u16>,
) -> anyhow::Result<(u16, Child, Child)> {
    let mut config = match read_config(sandbox_path.clone())? {
        Some(c) => c,
        None => panic!("invalid sandbox path {:?}", sandbox_path),
    };
    match force_admin_port {
        Some(port) => {
            set_admin_port(&mut config, port);
        }
        None => random_admin_port(&mut config),
    }
    let config_path = write_config(sandbox_path.clone(), &config);
    let (tx_config, rx_config) = oneshot::channel();
    let (mut child, lair) = start_holochain(holochain_path, config_path, tx_config).await?;
    check_started(&mut child).await;
    let port = rx_config
        .await
        .expect("Failed to get admin port from conductor");
    Ok((port, child, lair))
}

async fn start_holochain(
    holochain_path: &Path,
    config_path: PathBuf,
    tx_config: oneshot::Sender<u16>,
) -> anyhow::Result<(Child, Child)> {
    use tokio::io::AsyncWriteExt;
    let passphrase = holochain_util::pw::pw_get()?.read_lock().to_vec();

    let mut lair_path = config_path.clone();
    lair_path.pop();
    lair_path.push("keystore");

    tracing::info!("\n\n----\nstarting lair\n----\n\n");
    let mut cmd = Command::new("lair-keystore");
    cmd.arg("--lair-root")
        .arg(lair_path)
        .arg("server")
        .arg("--piped")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    msg!("{:?}", cmd);

    let mut lair = cmd.spawn().expect("Failed to spawn lair-keystore");

    let mut stdin = lair.stdin.take().unwrap();
    stdin.write_all(&passphrase).await?;
    stdin.shutdown().await?;
    drop(stdin);

    check_lair_running(lair.stdout.take().unwrap()).await;

    tracing::info!("\n\n----\nstarting holochain\n----\n\n");
    let mut cmd = Command::new(holochain_path);
    cmd.arg("--structured")
        // .env("RUST_LOG", "trace")
        .arg("--piped")
        .arg("--config-path")
        .arg(config_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    msg!("{:?}", cmd);

    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");

    let mut stdin = holochain.stdin.take().unwrap();
    stdin.write_all(&passphrase).await?;
    stdin.shutdown().await?;
    drop(stdin);

    // TODO: Allow redirecting output per conductor.
    spawn_output(&mut holochain, tx_config);
    Ok((holochain, lair))
}

async fn check_lair_running(stdout: tokio::process::ChildStdout) {
    let (s, r) = tokio::sync::oneshot::channel();
    let mut s = Some(s);
    tokio::task::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            println!("{}", line);
            if line == "# lair-keystore running #" {
                if let Some(s) = s.take() {
                    let _ = s.send(());
                }
            }
        }
    });
    let _ = r.await;
}

// TODO: Find a better way to confirm the child is running.
async fn check_started(holochain: &mut Child) {
    let started =
        tokio::time::timeout(std::time::Duration::from_millis(20), holochain.wait()).await;
    if let Ok(status) = started {
        panic!("Holochain failed to start. status: {:?}", status);
    }
}

fn spawn_output(holochain: &mut Child, config: oneshot::Sender<u16>) {
    let stdout = holochain.stdout.take();
    let stderr = holochain.stderr.take();
    tokio::task::spawn(async move {
        let mut needs_setup = true;
        let mut config = Some(config);
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if needs_setup {
                    match check_sandbox(&line, &mut needs_setup) {
                        (true, Some(port)) => {
                            if let Some(config) = config.take() {
                                config
                                    .send(port)
                                    .expect("Failed to send admin port from config");
                            }
                            continue;
                        }
                        (true, None) => continue,
                        (false, _) => (),
                    }
                }
                println!("{}", line);
            }
        }
    });
    tokio::task::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("{}", line);
            }
        }
    });
}

fn check_sandbox(line: &str, needs_setup: &mut bool) -> (bool, Option<u16>) {
    if let Some(line) = line.strip_prefix("###") {
        if let Some(line) = line.strip_suffix("###") {
            match line {
                "HOLOCHAIN_SANDBOX" => tracing::info!("Found config"),
                "HOLOCHAIN_SANDBOX_END" => *needs_setup = false,
                _ => {
                    if let Some(v) = line.strip_prefix("ADMIN_PORT:") {
                        if let Ok(port) = v.parse::<u16>() {
                            return (true, Some(port));
                        }
                    }
                }
            }
            return (true, None);
        }
    }
    (false, None)
}
