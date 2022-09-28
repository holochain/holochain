use crate::conductor::handle::DevSettingsDelta;
use crate::sweettest::*;
use crate::test_utils::conductor_setup::ConductorTestData;
use crate::test_utils::consistency_10s;
use crate::test_utils::inline_zomes::simple_create_read_zome;
use hdk::prelude::*;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::fresh_reader_test;
use holochain_test_wasm_common::AnchorInput;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p::KitsuneP2pConfig;

#[tokio::test(flavor = "multi_thread")]
async fn gossip_test() {
    observability::test_run().ok();
    let mut conductors = SweetConductorBatch::from_standard_config(2).await;

    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Anchor]).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((cell_1,), (cell_2,)) = apps.into_tuples();
    conductors.exchange_peer_info().await;

    let anchor = AnchorInput("alice".to_string(), "0".to_string());
    let _: EntryHash = conductors[0]
        .call(&cell_1.zome(TestWasm::Anchor), "anchor", anchor)
        .await;

    consistency_10s(&[&cell_1, &cell_2]).await;

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

    consistency_10s(&[&cell_1, &cell_2]).await;
    for p2p_agents_db in p2p_agents_dbs {
        let len = fresh_reader_test(p2p_agents_db.clone(), |txn| {
            txn.p2p_list_agents().unwrap().len()
        });
        assert_eq!(len, 2);
    }
}
