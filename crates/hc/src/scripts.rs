//! Common use setups with lots of default choices.
use std::path::Path;
use std::path::PathBuf;

use crate::calls::InstallApp;
use crate::cmds::*;
use crate::run::run_async;
use crate::CmdRunner;

/// Generates a new setup with a default [`ConductorConfig`]
/// and optional network.
/// Then installs the dnas with a new app per dna.
pub async fn default_with_network(
    holochain_path: &Path,
    create: Create,
    directory: Option<PathBuf>,
    dnas: Vec<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let Create {
        network,
        app_id,
        root,
        ..
    } = create;
    let path = crate::generate::generate(network.map(|n| n.into_inner().into()), root, directory)?;
    let conductor = run_async(holochain_path, path.clone(), None).await?;
    let mut cmd = CmdRunner::new(conductor.0).await;
    let install_app = InstallApp {
        app_id,
        agent_key: None,
        dnas,
    };
    crate::calls::install_app(&mut cmd, install_app).await?;
    Ok(path)
}

/// Same as [`default_with_network`] but creates n copies
/// of this setup in thier own directory.
pub async fn default_n(
    holochain_path: &Path,
    n: usize,
    create: Create,
    dnas: Vec<PathBuf>,
) -> anyhow::Result<Vec<PathBuf>> {
    msg!("Creating {} conductors with same settings", n);
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let p = default_with_network(
            holochain_path,
            create.clone(),
            create.directories.get(i).cloned(),
            dnas.clone(),
        )
        .await?;
        paths.push(p);
    }
    msg!("Created {:?}", paths);
    Ok(paths)
}
