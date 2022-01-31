use crate::test_utils::conductor_setup::ConductorTestData;
use crate::test_utils::consistency_envs;
use crate::test_utils::new_zome_call;
use hdk::prelude::*;
use holochain_p2p::{AgentPubKeyExt, DnaHashExt};
use holochain_sqlite::prelude::*;
use holochain_state::prelude::fresh_reader_test;
use holochain_test_wasm_common::AnchorInput;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p::KitsuneP2pConfig;
use matches::assert_matches;

#[tokio::test(flavor = "multi_thread")]
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
        new_zome_call(cell_id, "anchor", anchor, TestWasm::Anchor)
    };

    for i in 0..NUM {
        let invocation = anchor_invocation("alice", alice_cell_id, i).unwrap();
        let response = handle.call_zome(invocation).await.unwrap().unwrap();
        assert_matches!(response, ZomeCallResponse::Ok(_));
    }

    // Give publish time to finish
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Bring Bob online
    conductor_test.bring_bob_online().await;
    let bob_call_data = conductor_test.bob_call_data().unwrap();
    let bob_cell_id = &bob_call_data.cell_id;

    // Give gossip some time to finish
    const NUM_ATTEMPTS: usize = 200;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);

    let all_cell_envs = vec![
        (
            bob_call_data.cell_id.agent_pubkey(),
            &bob_call_data.authored_env,
            &bob_call_data.dht_env,
        ),
        (
            conductor_test.alice_call_data().cell_id.agent_pubkey(),
            &conductor_test.alice_call_data().authored_env,
            &conductor_test.alice_call_data().dht_env,
        ),
    ];
    consistency_envs(&all_cell_envs, NUM_ATTEMPTS, DELAY_PER_ATTEMPT).await;

    // Bob list anchors
    let invocation = new_zome_call(
        bob_cell_id,
        "list_anchor_addresses",
        "alice".to_string(),
        TestWasm::Anchor,
    )
    .unwrap();
    let response = handle.call_zome(invocation).await.unwrap().unwrap();
    match response {
        ZomeCallResponse::Ok(r) => {
            let hashes: EntryHashes = r.decode().unwrap();
            assert_eq!(hashes.0.len(), NUM);
        }
        _ => unreachable!(),
    }

    conductor_test.shutdown_conductor().await;
}

#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
// TODO: Rewrite this using sweettest.
// The idea of this test seems to be checking agent info is gossiped?
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
    let dna_kit = alice_call_data.ribosome.dna_file.dna_hash().to_kitsune();

    let alice_kit = alice_agent_id.to_kitsune();

    let p2p_env = handle.get_p2p_env(dna_kit.clone());

    let (agent_info, len) = fresh_reader_test(p2p_env.clone(), |txn| {
        let agent_info = txn.p2p_get_agent(&alice_kit).unwrap();
        let len = txn.p2p_list_agents().unwrap().len();
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
    let bob_kit = bob_agent_id.to_kitsune();

    // Give publish time to finish
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let (alice_agent_info, bob_agent_info, len) = fresh_reader_test(p2p_env.clone(), |txn| {
        let alice_agent_info = txn.p2p_get_agent(&alice_kit).unwrap();
        let bob_agent_info = txn.p2p_get_agent(&bob_kit).unwrap();
        let len = txn.p2p_list_agents().unwrap().len();
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
