//! Just commit large entries repeatedly. Intended to be used for generating a flamegraph
//! to diagnose why committing entries is slow.

#![allow(unused_imports)]

use std::io::Write;
use std::time::Instant;

use colored::*;
use holochain::sweettest::SweetConductorBatch;
use holochain_diagnostics::holochain::prelude::*;
use holochain_diagnostics::holochain::sweettest::{
    self, SweetConductor, SweetDnaFile, SweetInlineZomes,
};
use holochain_diagnostics::holochain::test_utils::inline_zomes::{simple_crud_zome, AppString};
use holochain_diagnostics::*;

#[tokio::main]
async fn main() {
    observability::test_run().ok();
    let start = Instant::now();

    // let config = config_no_networking();
    let config = config_standard();
    let mut conductors = SweetConductorBatch::from_config(2, config).await;
    conductors.exchange_peer_info().await;
    let conductor = &mut conductors[0];
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let app = conductor.setup_app("app", &[dna]).await.unwrap();
    let (cell,) = app.into_tuple();
    let zome = cell.zome(SweetInlineZomes::COORDINATOR);

    let setup_time = start.elapsed();
    println!("setup done in {:?}", setup_time);

    let mut rng = seeded_rng(None);

    let entry_size = 15_000_000;
    let mut total_committed = 0;

    // commit entries for roughly 10x as long as it took to setup the apps
    while start.elapsed().as_millis() < setup_time.as_millis() * 100 {
        let content = random_vec::<u8>(&mut rng, entry_size);
        let _: ActionHash = conductor.call(&zome, "create_bytes", content).await;
        print!(".");
        std::io::stdout().flush().ok();
        total_committed += 1;
    }
    let time = start.elapsed() - setup_time;
    println!();
    println!(
        "committed {} entries in {:?}. total throughput = {} MB/s",
        total_committed,
        time,
        (entry_size * total_committed) as f64 / time.as_micros() as f64,
    );
}
