use crate::sweettest::*;
use hdk::prelude::*;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_test_wasm_common::AnchorInput;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn gossip_test() {
    holochain_trace::test_run();
    let config =
        SweetConductorConfig::standard().tune_network_config(|nc| nc.disable_publish = true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Anchor]).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((cell_1,), (cell_2,)) = apps.into_tuples();

    let anchor = AnchorInput("alice".to_string(), "0".to_string());
    let _: EntryHash = conductors[0]
        .call(&cell_1.zome(TestWasm::Anchor), "anchor", anchor)
        .await;

    await_consistency(30, [&cell_1, &cell_2]).await.unwrap();

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
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;

    let mut config = ConductorConfig::default();
    // Hit a bootstrap service so it can blow up and return an error if we get our end of
    // things totally wrong.
    config.network.bootstrap_url = url2::url2!("{}", rendezvous.bootstrap_addr());
    let zomes = vec![TestWasm::Anchor];
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(zomes).await;
    let mut conductor = SweetConductor::from_config_rendezvous(config, rendezvous).await;

    conductor.setup_app("app", [&dna]).await.unwrap();

    // TODO should check that the app is running otherwise we don't know if bootstrap was called
    conductor.shutdown().await;
}
