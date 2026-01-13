use crate::sweettest::*;
use futures::StreamExt;
use holo_hash::ActionHash;
use holochain_wasm_test_utils::TestWasm;
use test_case::test_case;

#[test_case(2)]
#[test_case(4)]
#[tokio::test(flavor = "multi_thread")]
async fn conductors_call_remote(num_conductors: usize) {
    holochain_trace::test_run();

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let config = SweetConductorConfig::rendezvous(true);

    let mut conductors = SweetConductorBatch::from_config_rendezvous(num_conductors, config).await;

    let apps = conductors.setup_app("app", [&dna]).await.unwrap();
    let cells: Vec<_> = apps
        .into_inner()
        .into_iter()
        .map(|c| c.into_cells().into_iter().next().unwrap())
        .collect();

    await_consistency(cells.iter()).await.unwrap();

    let agents: Vec<_> = cells.iter().map(|c| c.agent_pubkey().clone()).collect();

    let iter = cells
        .clone()
        .into_iter()
        .zip(conductors.into_inner().into_iter());
    let keep = std::sync::Mutex::new(Vec::new());
    let keep = &keep;
    futures::stream::iter(iter)
        .for_each_concurrent(20, |(cell, conductor)| {
            let agents = agents.clone();
            async move {
                for agent in agents {
                    if agent == *cell.agent_pubkey() {
                        continue;
                    }
                    let _: ActionHash = conductor
                        .call(
                            &cell.zome(TestWasm::Create),
                            "call_create_entry_remotely_no_rec",
                            agent,
                        )
                        .await;
                }
                keep.lock().unwrap().push(conductor);
            }
        })
        .await;

    // Ensure that all the create requests were received and published.
    await_consistency(cells.iter()).await.unwrap();
}
