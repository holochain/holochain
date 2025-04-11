use holochain::sweettest::{SweetConductor, SweetDnaFile, SweetRendezvous};
use holochain_conductor_api::conductor::{ConductorConfig, NetworkConfig};
use holochain_serialized_bytes::SerializedBytes;
use holochain_wasm_test_utils::TestWasm;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::test(flavor = "multi_thread")]
async fn webrtc_connection() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();
    holochain_trace::test_run();
    let config = ConductorConfig {
        network: NetworkConfig {
            mem_bootstrap: false,
            ..Default::default()
        }
        .with_gossip_initiate_interval_ms(1000)
        .with_gossip_min_initiate_interval_ms(1000),
        ..Default::default()
    };
    println!("conductor config {config:?}");
    let mut conductor = SweetConductor::create_with_defaults_and_metrics(
        config,
        None,
        None::<Arc<dyn SweetRendezvous>>,
        false,
        true,
    )
    .await;
    let dna_file = SweetDnaFile::from_test_wasms(
        "8".to_string(),
        vec![TestWasm::Crd],
        SerializedBytes::default(),
    )
    .await
    .0;
    println!("dna hash {}", dna_file.dna_hash());
    let _app = conductor.setup_app("webrtc", &[dna_file]).await.unwrap();

    let instant = Instant::now();
    while instant.elapsed().as_secs() <= 180 {
        let agent_infos = conductor.get_agent_infos(None).await.unwrap();
        println!("agent_infos {:?}", agent_infos.len());
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
