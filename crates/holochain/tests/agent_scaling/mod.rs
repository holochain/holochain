#![cfg(feature = "test_utils")]

use futures::future;
use futures::FutureExt;
use hdk::prelude::GetLinksInputBuilder;
use holochain::sweettest::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
struct BaseTarget(AnyLinkableHash, AnyLinkableHash);

fn links_zome() -> InlineIntegrityZome {
    InlineIntegrityZome::new_unique(vec![], 1)
        .function("create_link", move |api, base_target: BaseTarget| {
            let hash = api.create_link(CreateLinkInput::new(
                base_target.0,
                base_target.1,
                ZomeIndex(0),
                LinkType::new(0),
                ().into(),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function(
            "get_links",
            move |api: BoxApi, base: AnyLinkableHash| -> InlineZomeResult<Vec<Vec<Link>>> {
                Ok(api.get_links(vec![GetLinksInputBuilder::try_new(
                    base,
                    InlineZomeSet::dep_link_filter(&api),
                )
                .unwrap()
                .build()])?)
            },
        )
}

/// A single link with an AgentPubKey for the base and target is committed by
/// one agent, and after a delay, all agents can get the link
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn many_agents_can_reach_consistency_agent_links() {
    holochain_trace::test_run();
    const NUM_AGENTS: usize = 20;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(("links", links_zome())).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    let apps = conductor
        .setup_apps("app", NUM_AGENTS, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let alice = cells[0].zome("links");

    // Must have integrated or be able to get the agent key to link from it
    await_consistency(10, &cells[..]).await.unwrap();

    let base: AnyLinkableHash = cells[0].agent_pubkey().clone().into();
    let target: AnyLinkableHash = cells[1].agent_pubkey().clone().into();

    let _: ActionHash = conductor
        .call(
            &alice,
            "create_link",
            BaseTarget(base.clone(), target.clone()),
        )
        .await;

    await_consistency(10, &cells[..]).await.unwrap();

    let mut seen = [0usize; NUM_AGENTS];

    for (i, cell) in cells.iter().enumerate() {
        let links: Vec<Vec<Link>> = conductor
            .call(&cell.zome("links"), "get_links", base.clone())
            .await;
        seen[i] = links.into_iter().next().unwrap().len();
    }

    assert_eq!(seen.to_vec(), [1; NUM_AGENTS].to_vec());
}

/// A single link with a Path for the base and target is committed by one
/// agent, and after a delay, all agents can get the link
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn many_agents_can_reach_consistency_normal_links() {
    holochain_trace::test_run();
    const NUM_AGENTS: usize = 30;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Link]).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    let apps = conductor
        .setup_apps("app", NUM_AGENTS, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let alice = cells[0].zome(TestWasm::Link);

    let _: ActionHash = conductor.call(&alice, "create_link", ()).await;

    await_consistency(10, &cells[..]).await.unwrap();

    let mut num_seen = 0;

    for cell in &cells {
        let links: Vec<Link> = conductor
            .call(&cell.zome(TestWasm::Link), "get_links", ())
            .await;
        num_seen += links.len();
    }

    assert_eq!(num_seen, NUM_AGENTS);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
// This could become a bench.
#[ignore = "Slow test for CI that is only useful for timing"]
async fn stuck_conductor_wasm_calls() -> anyhow::Result<()> {
    holochain_trace::test_run();
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MultipleCalls]).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    // Install DNA and install and enable apps in conductor
    let alice = conductor
        .setup_app("app", &[dna_file])
        .await
        .unwrap()
        .into_cells()
        .into_iter()
        .next()
        .unwrap();
    let alice = alice.zome(TestWasm::MultipleCalls);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
    pub struct TwoInt(pub u32, pub u32);

    // Make init run to avoid head moved errors
    let _: () = conductor.call(&alice, "slow_fn", TwoInt(0, 0)).await;

    let all_now = std::time::Instant::now();
    tracing::debug!("starting slow fn");

    // NB: there's currently no reason to independently create a bunch of tasks here,
    // since they are all running in series. Hence, there is no reason to put the SweetConductor
    // in an Arc. However, maybe this test was written to make it easy to try running some
    // or all of the closures concurrently, in which case the Arc is indeed necessary.
    let conductor_arc = std::sync::Arc::new(conductor);
    let mut handles = Vec::new();
    for i in 0..1000 {
        let h = tokio::task::spawn({
            let alice = alice.clone();
            let conductor = conductor_arc.clone();
            async move {
                let now = std::time::Instant::now();
                tracing::debug!("starting slow fn {}", i);
                let _: () = conductor.call(&alice, "slow_fn", TwoInt(i, 5)).await;
                tracing::debug!("finished slow fn {} in {}", i, now.elapsed().as_secs());
            }
        });
        handles.push(h);
    }

    for h in handles {
        h.await.unwrap();
    }

    tracing::debug!("finished all slow fn in {}", all_now.elapsed().as_secs());

    Ok(())
}

/// Check that many agents on the same conductor can all make lots of zome calls at once
/// without causing extremely ill effects
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
#[ignore = "performance test meant to be run manually"]
async fn many_concurrent_zome_calls_dont_gunk_up_the_works() {
    use holochain_conductor_api::{AppRequest, AppResponse, ZomeCall};
    use std::time::Instant;

    holochain_trace::test_run();
    const NUM_AGENTS: usize = 30;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MultipleCalls]).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    let apps = conductor
        .setup_apps("app", NUM_AGENTS, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let zomes: Vec<_> = cells
        .iter()
        .map(|c| c.zome(TestWasm::MultipleCalls))
        .collect();
    let mut clients: Vec<_> =
        future::join_all((0..NUM_AGENTS).map(|_| conductor.app_ws_client().map(|(tx, _)| tx)))
            .await;

    async fn all_call(
        conductor: &SweetConductor,
        zomes: &[holochain::sweettest::SweetZome],
        n: u32,
    ) {
        let start = Instant::now();
        let calls = future::join_all(zomes.iter().map(|zome| {
            conductor
                .call::<_, ()>(zome, "create_entry_multiple", n)
                .map(|r| (r, Instant::now()))
        }))
        .await;

        assert_eq!(calls.len(), NUM_AGENTS);

        for (i, (_, time)) in calls.iter().enumerate() {
            println!("{:>3}: {:?}", i, time.duration_since(start));
        }
    }

    async fn call_all_ws(
        conductor: &SweetConductor,
        cells: &[SweetCell],
        clients: &mut [holochain_websocket::WebsocketSender],
        n: u32,
    ) {
        let calls = future::join_all(std::iter::zip(cells, clients.iter_mut()).map(
            |(cell, client)| async move {
                let (nonce, expires_at) = holochain_nonce::fresh_nonce(Timestamp::now()).unwrap();
                let cell_id = cell.cell_id().clone();
                let call = ZomeCall::try_from_unsigned_zome_call(
                    conductor.raw_handle().keystore(),
                    ZomeCallUnsigned {
                        cell_id: cell_id.clone(),
                        zome_name: TestWasm::MultipleCalls.into(),
                        fn_name: "create_entry_multiple".into(),
                        cap_secret: None,
                        provenance: cell_id.agent_pubkey().clone(),
                        payload: ExternIO::encode(n).unwrap(),
                        nonce,
                        expires_at,
                    },
                )
                .await
                .unwrap();

                let start = Instant::now();
                let res: AppResponse = client
                    .request(AppRequest::CallZome(Box::new(call)))
                    .await
                    .unwrap();
                match res {
                    AppResponse::ZomeCalled(_) => Instant::now().duration_since(start),
                    other => panic!("unexpected ws response: {:?}", other),
                }
            },
        ))
        .await;

        for (i, duration) in calls.iter().enumerate() {
            println!("{:>3}: {:?}", i, duration);
        }
    }

    println!("----------------------");
    call_all_ws(&conductor, &cells, clients.as_mut_slice(), 10).await;
    println!("----------------------");
    call_all_ws(&conductor, &cells, clients.as_mut_slice(), 100).await;
    println!("----------------------");
    call_all_ws(&conductor, &cells, clients.as_mut_slice(), 100).await;

    for _ in 0..10 {
        println!("----------------------");
        let start = Instant::now();
        all_call(&conductor, &zomes, 100).await;
        println!("overall {:?}", Instant::now().duration_since(start));
    }
}
