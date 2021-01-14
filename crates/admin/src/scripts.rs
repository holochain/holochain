use std::path::PathBuf;
use structopt::StructOpt;

pub use cmds::*;
mod cmds;

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

pub async fn default_with_network(create: Create) -> anyhow::Result<PathBuf> {
    let Create {
        dnas,
        network,
        app_id,
    } = create;
    let path = crate::create(network.map(|n| n.into())).await?;
    crate::install_app(path.clone(), dnas, app_id).await?;
    Ok(path)
}

pub async fn default_n(create: Create, n: usize) -> anyhow::Result<Vec<PathBuf>> {
    msg!("Creating {} conductors with same settings", n);
    let mut paths = Vec::with_capacity(n);
    for _ in 0..n {
        let p = default_with_network(create.clone()).await?;
        paths.push(p);
    }
    msg!("Created {:?}", paths);
    Ok(paths)
}
