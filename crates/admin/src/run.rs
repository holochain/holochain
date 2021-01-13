use std::{path::PathBuf, process::Stdio};

use tokio::process::{Child, Command};

use crate::app::attach_app_port;
use crate::config::*;
use crate::ports::random_admin_port_if_busy;

pub async fn run(
    path: PathBuf,
    app_port: Option<u16>,
    force_admin_port: Option<u16>,
) -> anyhow::Result<()> {
    let (port, holochain) = run_async(path.clone(), force_admin_port).await?;
    msg!("Running conductor on admin port {}", port);
    tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
    if let Some(app_port) = app_port {
        msg!("Attaching app port {}", app_port);
        attach_app_port(app_port, port).await?;
    }
    msg!("Connected successfully to a running holochain");
    let e = format!("Failed to run holochain at {}", path.display());

    holochain.await.expect(&e);
    Ok(())
}

pub async fn run_async(
    path: PathBuf,
    force_admin_port: Option<u16>,
) -> anyhow::Result<(u16, Child)> {
    let mut config = match read_config(path.clone())? {
        Some(c) => c,
        None => create_config(path.clone()),
    };
    let port = match force_admin_port {
        Some(port) => port,
        None => random_admin_port_if_busy(&mut config),
    };
    let config_path = write_config(path.clone(), &config);
    Ok((port, start_holochain(config_path).await))
}

async fn start_holochain(config_path: PathBuf) -> Child {
    tracing::info!("\n\n----\nstarting holochain\n----\n\n");
    let mut cmd = Command::new("holochain");
    cmd.arg("--structured")
        // .env("RUST_LOG", "trace")
        .arg("--config-path")
        .arg(config_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut holochain = cmd.spawn().expect("Failed to spawn holochain");
    // spawn_output(&mut holochain);
    check_started(&mut holochain).await;
    holochain
}
async fn check_started(holochain: &mut Child) {
    let started = tokio::time::timeout(std::time::Duration::from_secs(1), holochain).await;
    if let Ok(status) = started {
        panic!("Holochain failed to start. status: {:?}", status);
    }
}
