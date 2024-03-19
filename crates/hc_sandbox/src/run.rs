//! Helpers for running the conductor.

use anyhow::anyhow;
use std::path::Path;
use std::process::Stdio;

use holochain_conductor_api::conductor::paths::ConfigFilePath;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::conductor::paths::KeystorePath;
use holochain_conductor_api::conductor::{ConductorConfig, KeystoreConfig};
use holochain_trace::Output;
use holochain_types::websocket::AllowedOrigins;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::calls::attach_app_interface;
use crate::calls::AddAppWs;
use crate::cli::LaunchInfo;
use crate::config::*;
use crate::ports::random_admin_port;
use crate::ports::set_admin_port;
use crate::CmdRunner;

// MAYBE: Export these strings from their respective repos
//        so that we can be sure to keep them in sync.
const LAIR_START: &str = "# lair-keystore running #";
const HC_START_1: &str = "HOLOCHAIN_SANDBOX";
const HC_START_2: &str = "HOLOCHAIN_SANDBOX_END";

/// Run a conductor and wait for it to finish.
/// Use [`run_async`] to run in the background.
/// Requires the holochain binary to be available
/// on the `holochain_path`.
/// Uses the sandbox provided by the `sandbox_path`.
/// Adds an app interface specified in the `app_ports`.
/// Can optionally force the admin port used. Otherwise
/// the port in the config will be used if it's free or
/// a random free port will be chosen.
pub async fn run(
    holochain_path: &Path,
    sandbox_path: ConfigRootPath,
    conductor_index: usize,
    app_ports: Vec<u16>,
    force_admin_port: Option<u16>,
    structured: Output,
) -> anyhow::Result<()> {
    let (admin_port, mut holochain, lair) = run_async(
        holochain_path,
        sandbox_path.clone(),
        force_admin_port,
        structured,
    )
    .await?;
    let mut launch_info = LaunchInfo::from_admin_port(admin_port);
    for app_port in app_ports {
        let mut cmd = CmdRunner::try_new(admin_port).await?;
        let port = attach_app_interface(
            &mut cmd,
            AddAppWs {
                port: Some(app_port),
                allowed_origins: AllowedOrigins::Any,
            },
        )
        .await?;
        launch_info.app_ports.push(port);
    }

    msg!(
        "Conductor launched #!{} {}",
        conductor_index,
        serde_json::to_string(&launch_info)?
    );

    crate::save::lock_live(std::env::current_dir()?, &sandbox_path, admin_port).await?;
    msg!("Connected successfully to a running holochain");
    let e = format!("Failed to run holochain at {}", sandbox_path.display());

    holochain.wait().await.expect(&e);
    if let Some(mut lair) = lair {
        let _ = lair.kill().await;
        lair.wait().await.expect("Failed to wait on lair-keystore");
    }

    Ok(())
}

/// Run a conductor in the background.
/// Requires the holochain binary to be available
/// on the `holochain_path`.
/// Uses the sandbox provided by the `sandbox_path`.
/// Can optionally force the admin port used. Otherwise
/// the port in the config will be used if it's free or
/// a random free port will be chosen.
pub async fn run_async(
    holochain_path: &Path,
    config_root_path: ConfigRootPath,
    force_admin_port: Option<u16>,
    structured: Output,
) -> anyhow::Result<(u16, Child, Option<Child>)> {
    let mut config = match read_config(config_root_path.clone())? {
        Some(c) => c,
        None => {
            let passphrase = holochain_util::pw::pw_get()?;
            let con_url = crate::generate::init_lair(
                &config_root_path.is_also_data_root_path().try_into()?,
                passphrase,
            )?;
            create_config(config_root_path.clone(), Some(con_url))?
        }
    };
    match force_admin_port {
        Some(port) => {
            set_admin_port(&mut config, port);
        }
        None => random_admin_port(&mut config),
    }
    let _config_file_path = write_config(config_root_path.clone(), &config);
    let (tx_config, rx_config) = oneshot::channel();
    let (child, lair) = start_holochain(
        holochain_path,
        &config,
        config_root_path,
        structured,
        tx_config,
    )
    .await?;

    let port = match rx_config.await {
        Ok(port) => port,
        Err(_) => {
            // We know this here because the sender has dropped which should only happen
            // if the spawned task that is scanning Holochain output has stopped
            return Err(anyhow!("Holochain process has exited"));
        }
    };

    Ok((port, child, lair))
}

async fn start_holochain(
    holochain_path: &Path,
    config: &ConductorConfig,
    config_root_path: ConfigRootPath,
    structured: Output,
    tx_config: oneshot::Sender<u16>,
) -> anyhow::Result<(Child, Option<Child>)> {
    use tokio::io::AsyncWriteExt;
    let passphrase = holochain_util::pw::pw_get()?.read_lock().to_vec();

    let lair = match config.keystore {
        KeystoreConfig::LairServer { .. } => {
            let lair = start_lair(
                passphrase.as_slice(),
                config_root_path.is_also_data_root_path().try_into()?,
            )
            .await?;
            Some(lair)
        }
        _ => None,
    };

    tracing::info!("\n\n----\nstarting holochain\n----\n\n");
    let mut cmd = Command::new(holochain_path);
    cmd.arg("--piped")
        .arg(format!("--structured={}", structured))
        .arg("--config-path")
        .arg(ConfigFilePath::from(config_root_path).as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");

    let mut stdin = holochain.stdin.take().unwrap();
    stdin.write_all(&passphrase).await?;
    stdin.shutdown().await?;
    drop(stdin);

    // TODO: Allow redirecting output per conductor.
    spawn_output(&mut holochain, tx_config);
    Ok((holochain, lair))
}

async fn start_lair(passphrase: &[u8], lair_path: KeystorePath) -> anyhow::Result<Child> {
    use tokio::io::AsyncWriteExt;

    tracing::info!("\n\n----\nstarting lair\n----\n\n");
    let mut cmd = Command::new("lair-keystore");
    cmd.arg("--lair-root")
        .arg(lair_path.as_ref())
        .arg("server")
        .arg("--piped")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    msg!("{:?}", cmd);

    let mut lair = cmd.spawn().expect("Failed to spawn lair-keystore");

    let mut stdin = lair.stdin.take().unwrap();
    stdin.write_all(passphrase).await?;
    stdin.shutdown().await?;
    drop(stdin);

    check_lair_running(lair.stdout.take().unwrap()).await;
    Ok(lair)
}

async fn check_lair_running(stdout: tokio::process::ChildStdout) {
    let (s, r) = oneshot::channel();
    let mut s = Some(s);
    tokio::task::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            println!("{}", line);
            if line == LAIR_START {
                if let Some(s) = s.take() {
                    let _ = s.send(());
                }
            }
        }
    });
    let _ = r.await;
}

fn spawn_output(holochain: &mut Child, config: oneshot::Sender<u16>) {
    let stdout = holochain.stdout.take();
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
}

fn check_sandbox(line: &str, needs_setup: &mut bool) -> (bool, Option<u16>) {
    if let Some(line) = line.strip_prefix("###") {
        if let Some(line) = line.strip_suffix("###") {
            match line {
                HC_START_1 => tracing::info!("Found config"),
                HC_START_2 => *needs_setup = false,
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
