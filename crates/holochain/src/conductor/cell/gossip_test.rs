use crate::conductor::handle::DevSettingsDelta;
use crate::sweettest::*;
use crate::test_utils::conductor_setup::ConductorTestData;
use crate::test_utils::consistency_10s;
use crate::test_utils::consistency_envs;
use crate::test_utils::inline_zomes::simple_create_read_zome;
use crate::test_utils::new_zome_call;
use hdk::prelude::*;
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
async fn agent_info_test() {
    observability::test_run().ok();
    let mut conductors = SweetConductorBatch::from_standard_config(2).await;

    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }
    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((cell_1,), (cell_2,)) = apps.into_tuples();
    conductors.exchange_peer_info().await;

    let p2p_envs: Vec<_> = conductors
        .iter()
        .filter_map(|c| {
            let lock = c.envs().p2p();
            let env = lock.lock().values().cloned().next();
            env
        })
        .collect();

    consistency_10s(&[&cell_1, &cell_2]).await;
    for p2p_env in &p2p_envs {
        let len = fresh_reader_test(p2p_env.clone(), |txn| txn.p2p_list_agents().unwrap().len());
        assert_eq!(len, 2);
    }
}
