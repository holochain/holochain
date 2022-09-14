#![cfg(feature = "test_utils")]

use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetDnaFile;
use holochain::test_utils::consistency_10s;
use holochain_serialized_bytes::prelude::*;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::inline_zome::BoxApi;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
struct BaseTarget(AnyLinkableHash, AnyLinkableHash);

fn links_zome() -> InlineZomeSet {
    InlineZomeSet::new_unique([("integrity_links", vec![], 1)], ["links"])
        .function(
            "links",
            "create_link",
            move |api, base_target: BaseTarget| {
                let hash = api.create_link(CreateLinkInput::new(
                    base_target.0,
                    base_target.1,
                    ZomeId(0),
                    LinkType::new(0),
                    ().into(),
                    ChainTopOrdering::default(),
                ))?;
                Ok(hash)
            },
        )
        .function(
            "links",
            "get_links",
            move |api: BoxApi, base: AnyLinkableHash| -> InlineZomeResult<Vec<Vec<Link>>> {
                Ok(api.get_links(vec![GetLinksInput::new(
                    base,
                    InlineZomeSet::dep_link_filter(&api),
                    None,
                )])?)
            },
        )
}

/// A single link with an AgentPubKey for the base and target is committed by
/// one agent, and after a delay, all agents can get the link
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn many_agents_can_reach_consistency_agent_links() {
    observability::test_run().ok();
    const NUM_AGENTS: usize = 20;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(links_zome())
        .await
        .unwrap();

    // Create a Conductor
    let mut conductor = SweetConductor::from_config(Default::default()).await;

    let agents = SweetAgents::get(conductor.keystore(), NUM_AGENTS).await;
    let apps = conductor
        .setup_app_for_agents("app", &agents, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let alice = cells[0].zome("links");

    // Must have integrated or be able to get the agent key to link from it
    consistency_10s(&cells[..]).await;

    let base: AnyLinkableHash = cells[0].agent_pubkey().clone().into();
    let target: AnyLinkableHash = cells[1].agent_pubkey().clone().into();

    let _: ActionHash = conductor
        .call(
            &alice,
            "create_link",
            BaseTarget(base.clone(), target.clone()),
        )
        .await;

    consistency_10s(&cells[..]).await;

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
    observability::test_run().ok();
    const NUM_AGENTS: usize = 30;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Link])
        .await
        .unwrap();

    // Create a Conductor
    let mut conductor = SweetConductor::from_config(Default::default()).await;

    let agents = SweetAgents::get(conductor.keystore(), NUM_AGENTS).await;
    let apps = conductor
        .setup_app_for_agents("app", &agents, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let alice = cells[0].zome(TestWasm::Link);

    let _: ActionHash = conductor.call(&alice, "create_link", ()).await;

    consistency_10s(&cells[..]).await;

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
    observability::test_run().ok();
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MultipleCalls]).await?;

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
