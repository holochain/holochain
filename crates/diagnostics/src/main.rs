use holochain::conductor::{state::ConductorState, ConductorStateDb};
// use holochain::core::state::source_chain::SourceChain;
use cell::dump_cell_state;
use conductor::dump_conductor_state;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_state::{
    db::{GetDb, CONDUCTOR_STATE},
    env::{EnvironmentKind, EnvironmentWrite},
    prelude::*,
};
use std::path::PathBuf;
use structopt::StructOpt;

mod cell;
mod conductor;
mod display;

#[derive(Debug, StructOpt)]
struct Opt {
    lmdb_path: PathBuf,
}

async fn run() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    // throwaway keystore that we'll never use.
    let keystore = spawn_test_keystore(Vec::new()).await.unwrap();

    // set up the various environments
    let conductor_env = EnvironmentWrite::new(
        opt.lmdb_path.as_ref(),
        EnvironmentKind::Conductor,
        keystore.clone(),
    )?;

    let conductor_state = dump_conductor_state(conductor_env).await?;

    for (_app_id, cells) in conductor_state.active_apps {
        for cell in cells {
            let (cell_id, cell_nick) = cell.into_inner();
            let cell_env = EnvironmentWrite::new(
                opt.lmdb_path.as_ref(),
                EnvironmentKind::Cell(cell_id.clone()),
                keystore.clone(),
            )?;
            dump_cell_state(cell_env, cell_id, &cell_nick).await?;
        }
    }

    Ok(())
}

#[tokio::main(threaded_scheduler)]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("holochain-analyzer: {}", err);
        std::process::exit(1);
    }
}
