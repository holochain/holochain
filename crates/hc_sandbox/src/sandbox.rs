//! Common use sandboxes with lots of default choices.

use crate::cmds::*;
use crate::run::run_async;
use holochain_client::AdminWebsocket;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::AppInfo;
use holochain_trace::Output;
use holochain_types::app::{AppManifest, RoleSettingsMap, RoleSettingsMapYaml};
use holochain_types::prelude::{
    AgentPubKey, AppBundleSource, InstallAppPayload, InstalledAppId, NetworkSeed,
};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// Install an app from a hApp bundle.
#[derive(Debug, Clone)]
pub struct InstallApp {
    /// Sets the InstalledAppId.
    pub app_id: Option<String>,

    /// If not set then a key will be generated.
    /// Agent key is Base64 (same format that is used in logs).
    pub agent_key: Option<AgentPubKey>,

    /// Location of the *.happ bundle file to install.
    pub path: PathBuf,

    /// Optional network seed override for every DNA in this app
    pub network_seed: Option<NetworkSeed>,

    /// Optional path to a yaml file containing role settings to override
    /// the values in the dna manifest(s).
    pub roles_settings: Option<PathBuf>,
}

/// Install an app bundle on the conductor.
async fn install_app_bundle(
    client: &mut AdminWebsocket,
    args: InstallApp,
) -> anyhow::Result<AppInfo> {
    let InstallApp {
        app_id,
        agent_key,
        path,
        network_seed,
        roles_settings,
    } = args;

    let roles_settings = match roles_settings {
        Some(path) => {
            let yaml_string = std::fs::read_to_string(path)?;
            let roles_settings_yaml = serde_yaml::from_str::<RoleSettingsMapYaml>(&yaml_string)?;
            let mut roles_settings: RoleSettingsMap = HashMap::new();
            for (k, v) in roles_settings_yaml.into_iter() {
                roles_settings.insert(k, v.into());
            }
            Some(roles_settings)
        }
        None => None,
    };

    let payload = InstallAppPayload {
        installed_app_id: app_id.clone(),
        agent_key,
        source: AppBundleSource::Path(path),
        roles_settings,
        network_seed,
        ignore_genesis_failure: false,
    };

    let installed_app = client.install_app(payload).await?;

    match &installed_app.manifest {
        AppManifest::V0(manifest) => {
            if !manifest.allow_deferred_memproofs {
                client
                    .enable_app(installed_app.installed_app_id.clone())
                    .await?;
            }
        }
    }
    Ok(installed_app)
}

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
    install_app_bundle(&mut client, install_bundle).await?;
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
    let num_sandboxes = create.num_sandboxes.into();
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
    Ok(paths)
}
