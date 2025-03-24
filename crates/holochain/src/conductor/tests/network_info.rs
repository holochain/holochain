use holochain_types::network::Kitsune2NetworkMetricsRequest;
use holochain_types::prelude::InstalledAppId;
use holochain_wasm_test_utils::TestWasm;

use crate::sweettest::*;

#[tokio::test(flavor = "multi_thread")]
async fn network_metrics() {
    holochain_trace::test_run();

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let number_of_peers = 3;
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductors = SweetConductorBatch::from_config(number_of_peers, config).await;
    let app_id: InstalledAppId = "app".into();
    conductors.setup_app(&app_id, &[dna.clone()]).await.unwrap();

    conductors.exchange_peer_info().await;

    let network_metrics = conductors[0]
        .dump_network_metrics_for_app(&app_id, Kitsune2NetworkMetricsRequest {
            dna_hash: Some(dna.dna_hash().clone()),
            include_dht_summary: false,
        })
        .await
        .unwrap();

    assert!(network_metrics.contains_key(dna.dna_hash()));

    let network_metrics = &network_metrics[dna.dna_hash()];
    assert_eq!(network_metrics.gossip_state_summary.peer_meta.len(), 2);
}
