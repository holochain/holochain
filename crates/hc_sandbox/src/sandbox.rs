//! Common use sandboxes with lots of default choices.

use holochain_client::AdminWebsocket;
use holochain_trace::Output;
use std::path::Path;
use std::path::PathBuf;

use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_types::prelude::InstalledAppId;

use crate::calls::InstallApp;
use crate::cmds::*;
use crate::run::run_async;

/// Generates a new sandbox with a default [`ConductorConfig`](holochain_conductor_api::config::conductor::ConductorConfig)
/// and optional network.
/// Then installs the specified hApp.
#[allow(clippy::too_many_arguments)]
pub async fn default_with_network(
    holochain_path: &Path,
    create: Create,
    directory: Option<PathBuf>,
    happ: PathBuf,
    app_id: InstalledAppId,
    network_seed: Option<String>,
    roles_settings: Option<PathBuf>,
    structured: Output,
) -> anyhow::Result<ConfigRootPath> {
    let Create {
        network,
        root,
        in_process_lair,
        #[cfg(feature = "chc")]
        chc_url,
        ..
    } = create;
    let network = Network::to_kitsune(&NetworkCmd::as_inner(&network)).await;
    let config_path = holochain_conductor_config::generate::generate(
        network,
        root,
        directory,
        in_process_lair,
        0,
        #[cfg(feature = "chc")]
        chc_url,
    )?;
    let conductor = run_async(holochain_path, config_path.clone(), None, structured).await?;
    let mut client = AdminWebsocket::connect(format!("localhost:{}", conductor.0), None).await?;
    let install_bundle = InstallApp {
        app_id: Some(app_id),
        agent_key: None,
        path: happ,
        network_seed,
        roles_settings,
    };
    crate::calls::install_app_bundle(&mut client, install_bundle).await?;
    Ok(config_path)
}

/// Same as [`default_with_network`] but creates _n_ copies
/// of this sandbox in separate directories.
pub async fn default_n(
    holochain_path: &Path,
    create: Create,
    happ: PathBuf,
    app_id: InstalledAppId,
    network_seed: Option<String>,
    roles_settings: Option<PathBuf>,
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
            roles_settings.clone(),
            structured.clone(),
        )
        .await?;
        paths.push(p);
    }
    msg!("Created {:?}", paths);
    Ok(paths)
}
