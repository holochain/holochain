use holochain::conductor::ConductorStateDb;
// use holochain::core::state::source_chain::SourceChain;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_state::{
    db::{GetDb, CONDUCTOR_STATE},
    env::{EnvironmentKind, EnvironmentWrite},
    prelude::*,
};
use std::path::PathBuf;
use structopt::StructOpt;

mod display;
use crate::display::human_size;

#[derive(Debug, StructOpt)]
struct Opt {
    lmdb_path: PathBuf,
}

async fn run() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let keystore = spawn_test_keystore(Vec::new()).await.unwrap();

    let conductor_env =
        EnvironmentWrite::new(opt.lmdb_path.as_ref(), EnvironmentKind::Conductor, keystore)?;

    get_conductor_state(conductor_env).await?;

    Ok(())
}

async fn get_conductor_state(env: EnvironmentWrite) -> anyhow::Result<()> {
    let g = env.guard().await;
    let r = g.reader()?;
    let db = ConductorStateDb::new(env.get_db(&CONDUCTOR_STATE)?)?;
    let bytes = db.get_bytes(&r, &().into())?.unwrap();
    let state = db.get(&r, &().into())?.unwrap();
    println!("+++++++ CONDUCTOR STATE +++++++");
    println!("Size: {}", human_size(bytes.len()));
    println!("Data: {:#?}", state);
    Ok(())
}

#[tokio::main(threaded_scheduler)]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("holochain-analyzer: {}", err);
        std::process::exit(1);
    }
}
