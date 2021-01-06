#![cfg(feature = "test_utils")]

use hdk3::prelude::Links;
use holochain::conductor::api::ZomeCall;
use holochain::test_utils::conductor_setup::ConductorTestData;
use holochain::test_utils::cool::CoolConductor;
use holochain::test_utils::cool::CoolDnaFile;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_lmdb::test_utils::test_environments;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

use unwrap_to::unwrap_to;

/// A single link with an AgentPubKey for the base and target is committed by
/// one agent, and after a delay, all agents can get the link
#[tokio::test(threaded_scheduler)]
#[cfg(feature = "slow_tests")]
async fn many_agents_can_reach_consistency_agent_links() {
    const NUM_AGENTS: usize = 20;
    let consistency_delay = std::time::Duration::from_secs(5);

    let envs = test_environments();
    let zomes = vec![TestWasm::Link];

    let dna_file = DnaFile::new(
        DnaDef {
            name: "conductor_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: zomes.clone().into_iter().map(Into::into).collect(),
        },
        zomes.into_iter().map(Into::into),
    )
    .await
    .unwrap();

    let mut agents = Vec::with_capacity(NUM_AGENTS);

    for _ in 0..NUM_AGENTS {
        agents.push(
            envs.keystore()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap(),
        )
    }

    let (mut conductor, cell_ids) =
        ConductorTestData::new(envs, vec![dna_file], agents, Default::default()).await;

    let cell_ids = cell_ids.values().next().unwrap();
    let committer = conductor.get_cell(&cell_ids[1]).unwrap();
    let base = cell_ids[0].agent_pubkey().clone();
    let target = cell_ids[1].agent_pubkey().clone();

    committer
        .get_api(TestWasm::Link)
        .create_link(base.clone().into(), target.into(), ().into())
        .await;

    committer.triggers.produce_dht_ops.trigger();

    tokio::time::delay_for(consistency_delay).await;
    let mut seen = [0; NUM_AGENTS];

    for i in 0..NUM_AGENTS {
        let cell_id = cell_ids[i].clone();
        let cd = conductor.get_cell(&cell_id).unwrap();

        let links = cd
            .get_api(TestWasm::Link)
            .get_links(base.clone().into(), None, Default::default())
            .await;

        seen[i] = links.len();
    }

    assert_eq!(seen.to_vec(), [1; NUM_AGENTS].to_vec());
}

/// A single link with a Path for the base and target is committed by one
/// agent, and after a delay, all agents can get the link
#[tokio::test(threaded_scheduler)]
#[cfg(feature = "slow_tests")]
async fn many_agents_can_reach_consistency_normal_links() {
    let num_agents = 30;
    let consistency_delay = std::time::Duration::from_secs(5);

    let envs = test_environments();
    let zomes = vec![TestWasm::Link];

    let dna_file = DnaFile::new(
        DnaDef {
            name: "conductor_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: zomes.clone().into_iter().map(Into::into).collect(),
        },
        zomes.into_iter().map(Into::into),
    )
    .await
    .unwrap();

    let mut agents = Vec::with_capacity(num_agents);

    for _ in 0..num_agents {
        agents.push(
            envs.keystore()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap(),
        )
    }

    let (conductor, cell_ids) =
        ConductorTestData::new(envs, vec![dna_file], agents, Default::default()).await;

    let cell_ids = cell_ids.values().next().unwrap();

    let _create_output = conductor
        .handle()
        .call_zome(ZomeCall {
            cell_id: cell_ids[0].clone(),
            zome_name: TestWasm::Link.into(),
            cap: None,
            fn_name: "create_link".into(),
            payload: ExternInput::new(SerializedBytes::try_from(()).unwrap()),
            provenance: cell_ids[0].agent_pubkey().clone(),
        })
        .await
        .unwrap()
        .unwrap();

    tokio::time::delay_for(consistency_delay).await;

    let mut num_seen = 0;

    for _ in 0..num_agents {
        let get_output = conductor
            .handle()
            .call_zome(ZomeCall {
                cell_id: cell_ids[1].clone(),
                zome_name: TestWasm::Link.into(),
                cap: None,
                fn_name: "get_links".into(),
                payload: ExternInput::new(SerializedBytes::try_from(()).unwrap()),
                provenance: cell_ids[1].agent_pubkey().clone(),
            })
            .await
            .unwrap()
            .unwrap();

        let links: Links = unwrap_to!(get_output => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .unwrap();

        num_seen += links.into_inner().len();
    }

    assert_eq!(num_seen, num_agents);
}

#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
#[ignore = "Slow test for CI that is only useful for timing"]
async fn stuck_conductor_wasm_calls() -> anyhow::Result<()> {
    observability::test_run().ok();
    // Bundle the single zome into a DnaFile
    let (dna_file, _) = CoolDnaFile::unique_from_test_wasms(vec![TestWasm::MultipleCalls]).await?;

    // Create a Conductor
    let mut conductor = CoolConductor::from_standard_config().await;

    // Install DNA and install and activate apps in conductor
    let alice = conductor
        .setup_app("app", &[dna_file])
        .await
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
    // since they are all running in series. Hence, there is no reason to put the CoolConductor
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
