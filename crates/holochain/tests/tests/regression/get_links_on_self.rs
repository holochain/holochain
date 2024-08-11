use std::time::Duration;

use hdk::prelude::*;
use holochain::sweettest::*;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn get_links_on_self() {
    holochain_trace::test_run();

    const N: usize = 10;
    const L: usize = 1;

    let config = SweetConductorConfig::rendezvous(true);
    let rendezvous = SweetLocalRendezvous::new().await;
    // let mut conductors = SweetConductorBatch::from_config_rendezvous(N, config).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Bulbasaur]).await;

    let mut conductors = Vec::new();
    let mut reserve =
        SweetConductor::from_config_rendezvous(config.clone(), rendezvous.clone()).await;
    let reserve_app = reserve.setup_app("", &[dna_file.clone()]).await.unwrap();
    let reserve_agent = reserve_app.agent().clone();
    // let mut apps = Vec::new();
    for i in 0..N {
        let mut conductor =
            SweetConductor::from_config_rendezvous(config.clone(), rendezvous.clone()).await;
        let app = conductor.setup_app("", &[dna_file.clone()]).await.unwrap();

        conductor
            .call::<_, ()>(
                &app.cells()[0].zome("bulbasaur"),
                "create_item",
                reserve_agent.clone(),
            )
            .await;

        tokio::time::sleep(Duration::from_secs(3)).await;

        conductors.push(conductor);
        // apps.push(app);
        println!("conductor {i} created");
    }

    let mut cells: Vec<_> = conductors
        .iter()
        .map(|conductor| {
            conductor
                .running_cell_ids()
                .into_iter()
                .map(|cell_id| conductor.get_sweet_cell(cell_id).unwrap())
        })
        .flatten()
        .collect();
    cells.push(
        reserve
            .get_sweet_cell(reserve_app.cells()[0].cell_id().clone())
            .unwrap(),
    );
    await_consistency(120, &cells).await.unwrap();

    // let cells = conductors
    //     .setup_app("app", &[dna_file])
    //     .await
    //     .unwrap()
    //     .cells_flattened();
    // let bobkey = cells[1].agent_pubkey().clone();

    // for _ in 0..L {
    //     let _: () = conductors[0]
    //         .call_fallible(&cells[0].zome("bulbasaur"), "create_item", bobkey.clone())
    //         .await
    //         .unwrap();
    // }

    // let mut done: HashSet<usize> = (0..conductors.len()).collect();
    // let mut times = vec![0; N];
    // let start = std::time::Instant::now();

    // while !done.is_empty() {
    //     for i in done.clone() {
    //         let links: Vec<Link> = conductors[i]
    //             .call_fallible(
    //                 &cells[i].zome("bulbasaur"),
    //                 "get_them_links",
    //                 bobkey.clone(),
    //             )
    //             .await
    //             .unwrap();
    //         if links.len() == L {
    //             done.remove(&i);
    //             times[i] = start.elapsed().as_millis();
    //         }
    //     }
    //     if !done.is_empty() {
    //         tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    //     }
    // }

    // println!("Time to complete for each node:\n{:?}", times);
}
