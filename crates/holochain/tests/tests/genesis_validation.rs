#[tokio::test(flavor = "multi_thread")]
async fn genesis_actions_are_validated_successfully() {
    use holo_hash::ActionHash;
    use holochain::sweettest::{
        await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
    };
    use holochain_wasm_test_utils::TestWasm;

    holochain_trace::test_run();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous(true))
            .await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna.clone()]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Carol installs the same DNA on the conductor that Alice is using
    let carol_app = conductors[0].setup_app("app2", &[dna]).await.unwrap();
    let carol = &carol_app.cells()[0];

    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();
    let carol_zome = carol.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&carol_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob, carol])
        .await
        .unwrap();
}
