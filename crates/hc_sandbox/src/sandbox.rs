//! Common use sandboxes with lots of default choices.
use std::path::Path;
use std::path::PathBuf;

use holochain_types::prelude::InstalledAppId;

use crate::bundles::DnasHapp;
use crate::calls::InstallApp;
use crate::calls::InstallAppBundle;
use crate::calls::RegisterDna;
use crate::cmds::*;
use crate::run::run_async;
use crate::CmdRunner;

/// Generates a new sandbox with a default [`ConductorConfig`]
/// and optional network.
/// Then installs the dnas with a new app per dna.
pub async fn default_with_network(
    holochain_path: &Path,
    create: Create,
    directory: Option<PathBuf>,
    to_install: DnasHapp,
    app_id: InstalledAppId,
) -> anyhow::Result<PathBuf> {
    let Create { network, root, .. } = create;
    let path = crate::generate::generate(network.map(|n| n.into_inner().into()), root, directory)?;
    let conductor = run_async(holochain_path, path.clone(), None).await?;
    let mut cmd = CmdRunner::new(conductor.0).await;
    match to_install {
        DnasHapp::Dnas(dnas) => {
            let mut hashes = Vec::with_capacity(dnas.len());
            for dna in dnas {
                let register_dna = RegisterDna {
                    uuid: None,
                    properties: None,
                    path: Some(dna),
                    hash: None,
                };
                let hash = crate::calls::register_dna(&mut cmd, register_dna).await?;
                hashes.push(hash);
            }
            let install_app = InstallApp {
                app_id,
                agent_key: None,
                dnas: hashes,
            };
            crate::calls::install_app(&mut cmd, install_app).await?;
        }
        DnasHapp::HApp(Some(happ)) => {
            let install_bundle = InstallAppBundle {
                app_id: Some(app_id),
                agent_key: None,
                path: happ,
            };
            crate::calls::install_app_bundle(&mut cmd, install_bundle).await?;
        }
        DnasHapp::HApp(None) => tracing::warn!("No happ found"),
    }
    Ok(path)
}

/// Same as [`default_with_network`] but creates n copies
/// of this sandbox in their own directories.
pub async fn default_n(
    holochain_path: &Path,
    create: Create,
    to_install: DnasHapp,
    app_id: InstalledAppId,
) -> anyhow::Result<Vec<PathBuf>> {
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
            to_install.clone(),
            app_id.clone(),
        )
        .await?;
        paths.push(p);
    }
    msg!("Created {:?}", paths);
    Ok(paths)
}
