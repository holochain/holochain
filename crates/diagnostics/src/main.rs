//! Holochain Diagnostics
//!
//! This is a simple program that spits out some info about LMDB databases.
//! It is written as a separate binary so that builds can be fast for quick
//! feedback loops when debugging. In the future, this could be reorganized
//! as a library of helper functions alongside a binary that calls into the lib,
//! so that the binary can be freely modifiable while still accumulating a
//! useful set of tools for querying LMDB state.

use cell::dump_cell_state;
use conductor::dump_conductor_state;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_sqlite::env::{EnvironmentKind, EnvironmentWrite};
use std::path::PathBuf;
use structopt::StructOpt;
use wasm::dump_wasm_state;

mod cell;
mod conductor;
mod display;
mod wasm;

#[derive(Debug, StructOpt)]
struct Opt {
    lmdb_path: PathBuf,
}

async fn run() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    // throwaway keystore that we'll never use.
    let keystore = spawn_test_keystore().await.unwrap();

    // set up the various environments
    let wasm_env = EnvironmentWrite::new(
        opt.lmdb_path.as_ref(),
        EnvironmentKind::Wasm,
        keystore.clone(),
    )?;

    println!();
    println!("        +++++++++++++++++++++++++++++++++");
    println!("        ++++++++   WASM  STATE   ++++++++");
    println!("        +++++++++++++++++++++++++++++++++");
    println!();
    dump_wasm_state(wasm_env).await?;

    // set up the various environments
    let conductor_env = EnvironmentWrite::new(
        opt.lmdb_path.as_ref(),
        EnvironmentKind::Conductor,
        keystore.clone(),
    )?;

    println!();
    println!("        +++++++++++++++++++++++++++++++++");
    println!("        +++++++  CONDUCTOR STATE  +++++++");
    println!("        +++++++++++++++++++++++++++++++++");
    println!();
    let conductor_state = dump_conductor_state(conductor_env).await?;

    println!();
    println!("        +++++++++++++++++++++++++++++++++");
    println!("        ++++++++   CELL  STATE   ++++++++");
    println!("        +++++++++++++++++++++++++++++++++");
    println!();

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
async fn main() -> anyhow::Result<()> {
    run().await
}
