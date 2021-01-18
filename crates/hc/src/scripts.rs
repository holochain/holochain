use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::cmds::*;

#[derive(Debug, StructOpt, Clone)]
pub enum Script {
    WithNetwork(Create),
    N {
        #[structopt(flatten)]
        create: Create,
        #[structopt(short, long)]
        num_conductors: usize,
    },
}

async fn default_with_network(
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
    let path = crate::create(network.map(|n| n.into_inner().into()), root, directory).await?;
    crate::install_app(holochain_path, path.clone(), dnas, app_id).await?;
    Ok(path)
}

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
