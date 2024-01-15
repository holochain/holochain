use crate::sweettest::*;
use crate::test_utils::conductor_setup::ConductorTestData;
use crate::test_utils::inline_zomes::simple_create_read_zome;
use crate::test_utils::{consistency_10s, consistency_60s};
use hdk::prelude::*;
use holochain_sqlite::store::AsP2pStateReadExt;
use holochain_test_wasm_common::AnchorInput;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p_types::config::{KitsuneP2pConfig, TransportConfig};

#[tokio::test(flavor = "multi_thread")]
async fn gossip_test() {
    holochain_trace::test_run().ok();
    let config = SweetConductorConfig::standard().no_publish();
    let mut conductors = SweetConductorBatch::from_config(2, config).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Anchor]).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((cell_1,), (cell_2,)) = apps.into_tuples();
    conductors.exchange_peer_info().await;

    let anchor = AnchorInput("alice".to_string(), "0".to_string());
    let _: EntryHash = conductors[0]
        .call(&cell_1.zome(TestWasm::Anchor), "anchor", anchor)
        .await;

    consistency_60s([&cell_1, &cell_2]).await;

    let hashes: EntryHashes = conductors[1]
        .call(
            &cell_2.zome(TestWasm::Anchor),
            "list_anchor_addresses",
            "alice",
        )
        .await;
    assert_eq!(hashes.0.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn signature_smoke_test() {
    holochain_trace::test_run().ok();

    let rendezvous = SweetLocalRendezvous::new().await;

    let mut network_config = KitsuneP2pConfig::default();
    network_config.transport_pool = vec![TransportConfig::Mem {}];
    // Hit a bootstrap service so it can blow up and return an error if we get our end of
    // things totally wrong.
    network_config.bootstrap_service = Some(url2::url2!("{}", rendezvous.bootstrap_addr()));
    let zomes = vec![TestWasm::Anchor];
    let mut conductor_test =
        ConductorTestData::with_network_config(zomes.clone(), false, network_config.clone()).await;
    // TODO should check that the app is running otherwise we don't know if bootstrap was called
    conductor_test.shutdown_conductor().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_info_test() {
    holochain_trace::test_run().ok();
    let config = SweetConductorConfig::standard().no_publish();
    let mut conductors = SweetConductorBatch::from_config(2, config).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("zome", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((cell_1,), (cell_2,)) = apps.into_tuples();
    conductors.exchange_peer_info().await;

    let p2p_agents_dbs: Vec<_> = conductors
        .iter()
        .filter_map(|c| {
            c.spaces
                .get_from_spaces(|s| s.p2p_agents_db.clone())
                .first()
                .cloned()
        })
        .collect();

    consistency_10s([&cell_1, &cell_2]).await;
    for p2p_agents_db in p2p_agents_dbs {
        let len = p2p_agents_db.p2p_count_agents().await.unwrap();
        assert_eq!(len, 2);
    }
}
