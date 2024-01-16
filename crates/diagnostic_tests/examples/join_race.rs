//! Testing join race condition
//!
//! - Spin up multiple conductors
//! - Install multiple apps on each
//! - Shut down and restart conductors until error manifests

use std::{io::Write, time::Duration};

use holochain_diagnostics::{
    holochain::{
        conductor::conductor::CellStatus, sweettest::*, test_utils::inline_zomes::simple_crud_zome,
    },
    seeded_rng, Rng, StdRng,
};

#[tokio::main]
async fn main() {
    holochain_trace::test_run().ok();

    // let config = config_no_networking();
    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(3, config).await;

    let (dna1, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let (dna3, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    conductors.setup_app("app1", &[dna1]).await.unwrap();
    conductors.setup_app("app2", &[dna2]).await.unwrap();
    conductors.setup_app("app3", &[dna3]).await.unwrap();

    fn random_duration(rng: &mut StdRng, max: u64) -> Duration {
        let min = 500;
        Duration::from_millis(rng.gen_range(min..=max))
    }

    let num_conductors = conductors.len();
    let tasks = conductors.into_iter().enumerate().map(|(i, mut c)| {
        tokio::task::spawn(async move {
            let mut rng = seeded_rng(None);
            let id = format!("{}{}{}", " ".repeat(i), i, " ".repeat(num_conductors - i));
            loop {
                let status: CellStatus = c.cell_status().values().cloned().next().unwrap();
                match status {
                    CellStatus::Joined => {
                        println!("{id} JOINED");
                        c.shutdown().await;
                        let shutdown_dur = random_duration(&mut rng, 2_000);
                        println!("{id} shut down, waiting {:?}", shutdown_dur);
                        tokio::time::sleep(shutdown_dur).await;
                        c.startup().await;
                        let startup_dur = random_duration(&mut rng, 5_000);
                        println!("{id} restarted, waiting {:?}", startup_dur);
                        tokio::time::sleep(startup_dur).await;
                    }
                    CellStatus::Joining => {
                        println!("{id} still joining, waiting 1 sec");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    CellStatus::PendingJoin(reason) => {
                        println!("{id} Failed to join: {:?}", reason);
                        panic!("{id} Failed to join: {:?}", reason);
                    }
                }
            }
        })
    });

    futures::future::join_all(tasks).await;
}
