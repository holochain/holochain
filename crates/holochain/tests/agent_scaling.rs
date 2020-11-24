#![cfg(feature = "test_utils")]

use hdk3::prelude::Links;
use holochain::nucleus::dna::DnaFile;
use holochain::nucleus::ribosome::ZomeCallInvocation;
use holochain::test_utils::conductor_setup::ConductorTestData;
use holochain::{nucleus::dna::DnaDef, prelude::Zome};
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_serialized_bytes::prelude::*;
use holochain_state::test_utils::test_environments;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::ExternInput;
use holochain_zome_types::ZomeCallResponse;
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
            zomes: zomes
                .clone()
                .into_iter()
                .map(|w| Zome::from(w).into_inner())
                .collect(),
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
            zomes: zomes
                .clone()
                .into_iter()
                .map(|w| Zome::from(w).into_inner())
                .collect(),
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
        .call_zome(ZomeCallInvocation {
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
            .call_zome(ZomeCallInvocation {
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
