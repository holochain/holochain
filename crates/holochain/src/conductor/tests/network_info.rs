use crate::prelude::fake_dna_hash;
use crate::sweettest::*;
use hdk::prelude::CloneCellId;
use hdk::prelude::DnaModifiersOpt;
use holochain_types::app::{CreateCloneCellPayload, EnableCloneCellPayload};
use holochain_types::network::Kitsune2NetworkMetricsRequest;
use holochain_types::prelude::InstalledAppId;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn network_metrics() {
    holochain_trace::test_run();

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let number_of_peers = 3;
    let config = SweetConductorConfig::standard();
    let mut conductors = SweetConductorBatch::from_config(number_of_peers, config).await;
    let app_id: InstalledAppId = "app".into();
    conductors.setup_app(&app_id, &[dna.clone()]).await.unwrap();

    conductors.exchange_peer_info().await;

    let network_metrics = conductors[0]
        .dump_network_metrics_for_app(
            &app_id,
            Kitsune2NetworkMetricsRequest {
                dna_hash: Some(dna.dna_hash().clone()),
                include_dht_summary: false,
            },
        )
        .await
        .unwrap();

    assert!(network_metrics.contains_key(dna.dna_hash()));

    let network_metrics = &network_metrics[dna.dna_hash()];
    assert_eq!(network_metrics.gossip_state_summary.peer_meta.len(), 2);

    let fake_dna = fake_dna_hash(1);
    let response = conductors[0]
        .dump_network_metrics_for_app(
            &app_id,
            Kitsune2NetworkMetricsRequest {
                dna_hash: Some(fake_dna),
                include_dht_summary: false,
            },
        )
        .await;
    assert!(response.is_err());

    // Create a disabled clone cell for app1
    let clone_cell = conductors[0]
        .create_clone_cell(
            &app_id,
            CreateCloneCellPayload {
                role_name: dna.dna_hash().to_string(),
                modifiers: DnaModifiersOpt::none().with_network_seed("test_seed".to_string()),
                membrane_proof: None,
                name: Some("disabled_clone".into()),
            },
        )
        .await
        .unwrap();

    let clone_cell_id = CloneCellId::CloneId(clone_cell.clone_id);
    let response = conductors[0]
        .clone()
        .enable_clone_cell(&app_id, &EnableCloneCellPayload { clone_cell_id })
        .await;
    assert!(!response.is_err());

    let response = conductors[0]
        .dump_network_metrics_for_app(
            &app_id,
            Kitsune2NetworkMetricsRequest {
                dna_hash: Some(clone_cell.cell_id.dna_hash().clone()),
                include_dht_summary: false,
            },
        )
        .await;
    assert!(response.unwrap().contains_key(clone_cell.cell_id.dna_hash()));
}

