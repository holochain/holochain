//! Helpers for running the conductor.
use std::path::Path;
use std::{path::PathBuf, process::Stdio};

use tokio::process::{Child, Command};

use crate::calls::attach_app_interface;
use crate::calls::AddAppWs;
use crate::config::*;
use crate::ports::random_admin_port_if_busy;
use crate::ports::set_admin_port;
use crate::CmdRunner;

/// Run a conductor and wait for it to finish.
/// Use [`run_async`] to run in the background.
/// Requires the holochain binary is available
/// on the `holochain_path`.
/// Uses the setup provided by the `setup_path`.
/// Adds an app interface in the `app_ports`.
/// Can optionally force the admin port used. Otherwise
/// the port in the config will be used if it's free or
/// a random free port will be chosen.
pub async fn run(
    holochain_path: &Path,
    setup_path: PathBuf,
    app_ports: Vec<u16>,
    force_admin_port: Option<u16>,
) -> anyhow::Result<()> {
    let (port, holochain) = run_async(holochain_path, setup_path.clone(), force_admin_port).await?;
    msg!("Running conductor on admin port {}", port);
    for app_port in app_ports {
        msg!("Attaching app port {}", app_port);
        let mut cmd = CmdRunner::try_new(port).await?;
        attach_app_interface(
            &mut cmd,
            AddAppWs {
                port: Some(app_port),
            },
        )
        .await?;
    }
    msg!("Connected successfully to a running holochain");
    let e = format!("Failed to run holochain at {}", setup_path.display());

    holochain.await.expect(&e);
    Ok(())
}

/// Run a conductor in the background.
/// Requires the holochain binary is available
/// on the `holochain_path`.
/// Uses the setup provided by the `setup_path`.
/// Can optionally force the admin port used. Otherwise
/// the port in the config will be used if it's free or
/// a random free port will be chosen.
pub async fn run_async(
    holochain_path: &Path,
    setup_path: PathBuf,
    force_admin_port: Option<u16>,
) -> anyhow::Result<(u16, Child)> {
    let mut config = match read_config(setup_path.clone())? {
        Some(c) => c,
        None => create_config(setup_path.clone()),
    };
    let port = match force_admin_port {
        Some(port) => {
            set_admin_port(&mut config, port);
            port
        }
        None => random_admin_port_if_busy(&mut config),
    };
    let config_path = write_config(setup_path.clone(), &config);
    Ok((port, start_holochain(holochain_path, config_path).await))
}

async fn start_holochain(holochain_path: &Path, config_path: PathBuf) -> Child {
    tracing::info!("\n\n----\nstarting holochain\n----\n\n");
    let mut cmd = Command::new(holochain_path);
    cmd.arg("--structured")
        // .env("RUST_LOG", "trace")
        .arg("--config-path")
        .arg(config_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    // TODO: Allow redirecting output per conductor.
    // spawn_output(&mut holochain);
    check_started(&mut holochain).await;
    holochain
}

// TODO: Find a better way to confirm the child is running.
async fn check_started(holochain: &mut Child) {
    let started = tokio::time::timeout(std::time::Duration::from_secs(1), holochain).await;
    if let Ok(status) = started {
        panic!("Holochain failed to start. status: {:?}", status);
    }
}
