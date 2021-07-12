use std::sync::Arc;

use hdk::prelude::*;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::consistency_10s;
use kitsune_p2p::KitsuneP2pConfig;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_gossip() -> anyhow::Result<()> {
    use holochain::test_utils::inline_zomes::simple_create_read_zome;

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 2;

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;
    for c in conductors.iter() {
        c.set_skip_publish(true);
    }

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;
    let all_cells = vec![&alice, &bobbo];

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductors[1].call(&bobbo.zome("zome1"), "read", hash).await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}
