//! Testing join race condition
//!
//! - Spin up multiple conductors
//! - Install multiple apps on each
//! - Shut down and restart conductors until error manifests

use std::time::Duration;

use holochain_diagnostics::{
    holochain::{
        conductor::conductor::CellStatus, sweettest::*, test_utils::inline_zomes::simple_crud_zome,
    },
    seeded_rng, Rng, StdRng,
};
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    holochain_trace::test_run().ok();

    const NUM: usize = 10;
    const MIN_WAIT_MS: Duration = Duration::from_millis(100);
    const MAX_WAIT_MS: Duration = Duration::from_millis(500);

    // let config = config_no_networking();
    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(NUM, config).await;

    let (dna1, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let (dna3, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    conductors.setup_app("app1", &[dna1]).await.unwrap();
    conductors.setup_app("app2", &[dna2]).await.unwrap();
    conductors.setup_app("app3", &[dna3]).await.unwrap();

    let start = Instant::now();

    fn random_duration(rng: &mut StdRng, max: u64) -> Duration {
        let min = MIN_WAIT_MS.as_millis() as u64;
        Duration::from_millis(rng.gen_range(min..=max))
    }

    let tasks = conductors
        .into_iter()
        .enumerate()
        .map(|(i, mut c)| {
            tokio::task::spawn(async move {
                let mut rng = seeded_rng(None);
                let id = format!("{}{}{}", " ".repeat(i), i, " ".repeat(NUM - i));
                loop {
                    let status: Vec<CellStatus> = c.cell_status().values().cloned().collect();
                    if let Some(fail) = status
                        .iter()
                        .find(|s| matches!(s, CellStatus::PendingJoin(_)))
                    {
                        return anyhow::Result::<()>::Err(anyhow::anyhow!(
                            "{id} Failed to join: {:?}",
                            fail
                        ));
                    } else if status.iter().all(|s| matches!(s, CellStatus::Joining)) {
                        println!("{id} still joining");
                        tokio::time::sleep(random_duration(&mut rng, 500)).await;
                    } else {
                        println!("{id} LIVE: {status:?}");

                        {
                            let t = Instant::now();
                            c.shutdown().await;
                            let shutdown_dur = random_duration(&mut rng, 500);
                            println!(
                                "{id} shut down in {:?}, waiting {:?}",
                                t.elapsed(),
                                shutdown_dur
                            );
                            tokio::time::sleep(shutdown_dur).await;
                        }
                        {
                            let t = Instant::now();
                            c.startup().await;
                            let startup_dur =
                                random_duration(&mut rng, MAX_WAIT_MS.as_millis() as u64);
                            let elapsed = t.elapsed();

                            if elapsed >= Duration::from_millis(500) {
                                println!(
                                    "\n{id}  !!!  restarted in {:?}, waiting {:?}  !!!\n",
                                    t.elapsed(),
                                    startup_dur
                                );
                            } else {
                                println!(
                                    "{id} restarted in {:?}, waiting {:?}",
                                    t.elapsed(),
                                    startup_dur
                                );
                            }

                            tokio::time::sleep(startup_dur).await;
                        }
                    }
                }
            })
        })
        .chain([tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                println!("{} {:?}", ".".repeat(NUM), start.elapsed());
            }
        })]);

    let (r, i, _tasks) = futures::future::select_all(tasks).await;
    println!("FAILURE in {} after {:?}", i, start.elapsed());
    r.unwrap().unwrap();
}
