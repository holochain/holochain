//! Common use sandboxes with lots of default choices.

use holochain_trace::Output;
use std::path::Path;
use std::path::PathBuf;

use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_types::prelude::InstalledAppId;

use crate::calls::InstallApp;
use crate::cmds::*;
use crate::run::run_async;
use crate::CmdRunner;

/// Generates a new sandbox with a default [`ConductorConfig`](holochain_conductor_api::config::conductor::ConductorConfig)
/// and optional network.
/// Then installs the specified hApp.
pub async fn default_with_network(
    holochain_path: &Path,
    create: Create,
    directory: Option<PathBuf>,
    happ: PathBuf,
    app_id: InstalledAppId,
    network_seed: Option<String>,
    structured: Output,
) -> anyhow::Result<ConfigRootPath> {
    let Create {
        network,
        root,
        in_process_lair,
        ..
    } = create;
    let network = Network::to_kitsune(&NetworkCmd::as_inner(&network)).await;
    let path = crate::generate::generate(network, root, directory, in_process_lair)?;
    let conductor = run_async(holochain_path, path.clone(), None, structured).await?;
    let mut cmd = CmdRunner::new(conductor.0).await;
    let install_bundle = InstallApp {
        app_id: Some(app_id),
        agent_key: None,
        path: happ,
        network_seed,
    };
    crate::calls::install_app_bundle(&mut cmd, install_bundle).await?;
    Ok(path)
}

/// Same as [`default_with_network`] but creates _n_ copies
/// of this sandbox in separate directories.
pub async fn default_n(
    holochain_path: &Path,
    create: Create,
    happ: PathBuf,
    app_id: InstalledAppId,
    network_seed: Option<String>,
    structured: Output,
) -> anyhow::Result<Vec<ConfigRootPath>> {
    let num_sandboxes = create.num_sandboxes;
    msg!(
        "Creating {} conductor sandboxes with same settings",
        num_sandboxes
    );
    let mut paths = Vec::with_capacity(num_sandboxes);
    for i in 0..num_sandboxes {
        let p = default_with_network(
            holochain_path,
            create.clone(),
            create.directories.get(i).cloned(),
            happ.clone(),
            app_id.clone(),
            network_seed.clone(),
            structured.clone(),
        )
        .await?;
        paths.push(p);
    }
    msg!("Created {:?}", paths);
    Ok(paths)
}
