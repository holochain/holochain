use crate::conductor::p2p_store::AgentKv;
use crate::conductor::p2p_store::AgentKvKey;
use crate::test_utils::conductor_setup::ConductorTestData;
use crate::test_utils::new_invocation;
use fallible_iterator::FallibleIterator;
use hdk3::prelude::*;
use holochain_state::buffer::KvStoreT;
use holochain_state::fresh_reader_test;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p::KitsuneBinType;
use kitsune_p2p::KitsuneP2pConfig;
use matches::assert_matches;
use test_wasm_common::AnchorInput;
use test_wasm_common::TestString;

#[tokio::test(threaded_scheduler)]
async fn gossip_test() {
    observability::test_run().ok();
    const NUM: usize = 1;
    let zomes = vec![TestWasm::Anchor];
    let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
    let handle = conductor_test.handle();
    let alice_call_data = &conductor_test.alice_call_data();
    let alice_cell_id = &alice_call_data.cell_id;

    // ALICE adding anchors

    let anchor_invocation = |anchor: &str, cell_id, i: usize| {
        let anchor = AnchorInput(anchor.into(), i.to_string());
        new_invocation(cell_id, "anchor", anchor, TestWasm::Anchor)
    };

    for i in 0..NUM {
        let invocation = anchor_invocation("alice", alice_cell_id, i).unwrap();
        let response = handle.call_zome(invocation).await.unwrap().unwrap();
        assert_matches!(response, ZomeCallResponse::Ok(_));
    }

    // Give publish time to finish
    tokio::time::delay_for(std::time::Duration::from_secs(1)).await;

    // Bring Bob online
    conductor_test.bring_bob_online().await;
    let bob_call_data = conductor_test.bob_call_data().unwrap();
    let bob_cell_id = &bob_call_data.cell_id;

    // Give gossip some time to finish
    tokio::time::delay_for(std::time::Duration::from_secs(1)).await;

    // Bob list anchors
    let invocation = new_invocation(
        bob_cell_id,
        "list_anchor_addresses",
        TestString("alice".into()),
        TestWasm::Anchor,
    )
    .unwrap();
    let response = handle.call_zome(invocation).await.unwrap().unwrap();
    match response {
        ZomeCallResponse::Ok(r) => {
            let response: SerializedBytes = r.into_inner();
            let hashes: EntryHashes = response.try_into().unwrap();
            assert_eq!(hashes.0.len(), NUM);
        }
        _ => unreachable!(),
    }

    conductor_test.shutdown_conductor().await;
}

#[tokio::test(threaded_scheduler)]
async fn signature_smoke_test() {
    observability::test_run().ok();
    let mut network_config = KitsuneP2pConfig::default();
    network_config.transport_pool = vec![kitsune_p2p::TransportConfig::Mem {}];
    // Hit an actual bootstrap service so it can blow up and return an error if we get our end of
    // things totally wrong.
    network_config.bootstrap_service = Some(url2::url2!("{}", kitsune_p2p::BOOTSTRAP_SERVICE_DEV));
    let zomes = vec![TestWasm::Anchor];
    let mut conductor_test =
        ConductorTestData::with_network_config(zomes.clone(), false, network_config.clone()).await;
    conductor_test.shutdown_conductor().await;
}

#[tokio::test(threaded_scheduler)]
#[ignore = "Conductors are not currently talking to each other"]
async fn agent_info_test() {
    observability::test_run().ok();
    let mut network_config = KitsuneP2pConfig::default();
    network_config.transport_pool = vec![kitsune_p2p::TransportConfig::Mem {}];
    let zomes = vec![TestWasm::Anchor];
    let mut conductor_test =
        ConductorTestData::with_network_config(zomes.clone(), false, network_config.clone()).await;
    let handle = conductor_test.handle();
    let alice_call_data = &conductor_test.alice_call_data();
    let alice_cell_id = &alice_call_data.cell_id;
    let alice_agent_id = alice_cell_id.agent_pubkey();

    // Kitsune types
    let dna_kit = kitsune_p2p::KitsuneSpace::new(
        alice_call_data
            .ribosome
            .dna_file
            .dna_hash()
            .get_raw_36()
            .to_vec(),
    );

    let alice_kit = kitsune_p2p::KitsuneAgent::new(alice_agent_id.get_raw_36().to_vec());

    let p2p_env = handle.get_p2p_env().await;
    let p2p_kv = AgentKv::new(p2p_env.clone().into()).unwrap();

    let alice_key: AgentKvKey = (&dna_kit, &alice_kit).into();

    let (agent_info, len) = fresh_reader_test!(p2p_env, |r| {
        let agent_info = p2p_kv.as_store_ref().get(&r, &alice_key).unwrap();
        let len = p2p_kv.as_store_ref().iter(&r).unwrap().count().unwrap();
        (agent_info, len)
    });
    tracing::debug!(?agent_info);
    assert_matches!(agent_info, Some(_));
    // Expecting one agent info in the peer store
    assert_eq!(len, 1);

    // Bring Bob online
    let mut bob_conductor =
        ConductorTestData::with_network_config(zomes, true, network_config.clone()).await;
    let bob_agent_id = bob_conductor
        .bob_call_data()
        .unwrap()
        .cell_id
        .agent_pubkey();
    let bob_kit = kitsune_p2p::KitsuneAgent::new(bob_agent_id.get_raw_36().to_vec());
    let bob_key: AgentKvKey = (&dna_kit, &bob_kit).into();

    // Give publish time to finish
    tokio::time::delay_for(std::time::Duration::from_secs(2)).await;

    let p2p_kv = AgentKv::new(p2p_env.clone().into()).unwrap();
    let (alice_agent_info, bob_agent_info, len) = fresh_reader_test!(p2p_env, |r| {
        let alice_agent_info = p2p_kv.as_store_ref().get(&r, &alice_key).unwrap();
        let bob_agent_info = p2p_kv.as_store_ref().get(&r, &bob_key).unwrap();
        let len = p2p_kv.as_store_ref().iter(&r).unwrap().count().unwrap();
        (alice_agent_info, bob_agent_info, len)
    });
    tracing::debug!(?alice_agent_info);
    tracing::debug!(?bob_agent_info);
    assert_matches!(alice_agent_info, Some(_));
    assert_matches!(bob_agent_info, Some(_));
    // Expecting one agent info in the peer store
    assert_eq!(len, 2);

    conductor_test.shutdown_conductor().await;
    bob_conductor.shutdown_conductor().await;
}
