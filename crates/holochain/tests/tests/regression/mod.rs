use holo_hash::ActionHash;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetAgents, SweetConductor, SweetConductorBatch, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
use kitsune_p2p_types::config::KitsuneP2pConfig;
use std::sync::Arc;

// Intended to keep https://github.com/holochain/holochain/issues/2868 fixed.
#[tokio::test(flavor = "multi_thread")]
async fn zome_with_no_entry_types_does_not_prevent_deletes() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ValidateRejectAppTypes, TestWasm::Crd])
            .await;
    let agent = SweetAgents::alice();

    let (cell,) = conductor
        .setup_app_for_agent("app", agent, &[dna_file])
        .await
        .unwrap()
        .into_tuple();

    let created: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Crd.coordinator_zome_name()),
            "create",
            (),
        )
        .await;

    let _: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Crd.coordinator_zome_name()),
            "delete_via_hash",
            created,
        )
        .await;
}

// Intended to keep https://github.com/holochain/holochain/issues/2868 fixed.
#[tokio::test(flavor = "multi_thread")]
async fn zome_with_no_link_types_does_not_prevent_delete_links() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
        TestWasm::ValidateRejectAppTypes,
        TestWasm::Link,
    ])
    .await;
    let agent = SweetAgents::alice();

    let (cell,) = conductor
        .setup_app_for_agent("app", agent, &[dna_file])
        .await
        .unwrap()
        .into_tuple();

    let created: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Link.coordinator_zome_name()),
            "create_link",
            (),
        )
        .await;

    let _: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Link.coordinator_zome_name()),
            "delete_link",
            created,
        )
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn zero_arc_does_not_prevent_delete_links() {
    use hdk::prelude::*;

    holochain_trace::test_run();

    let mut empty_arc_conductor_config = ConductorConfig::default();

    let mut network_config = KitsuneP2pConfig::default();

    let mut tuning_params = KitsuneP2pTuningParams::default();

    tuning_params.gossip_arc_clamping = String::from("empty");
    network_config.tuning_params = Arc::new(tuning_params);

    empty_arc_conductor_config.network = network_config;

    let mut conductors = SweetConductorBatch::from_configs(vec![
        ConductorConfig::default(),
        empty_arc_conductor_config,
    ])
    .await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
        TestWasm::ValidateRejectAppTypes,
        TestWasm::Link,
    ])
    .await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bob,)) = apps.into_tuples();

    let alice_pk = alice.cell_id().agent_pubkey().clone();
    let bob_pk = bob.cell_id().agent_pubkey().clone();

    println!("@!@!@ alice_pk: {alice_pk:?}, bob_pk: {bob_pk:?}");

    let action_hash: ActionHash = conductors[0]
        .call(
            &alice.zome(TestWasm::Link.coordinator_zome_name()),
            "test_entry_create",
            (),
        )
        .await;

    println!("@!@!@ action_hash: {action_hash:?}");

    loop {
        // Wait until we can get the action from alice
        // Since alice is the only node that holds DHT ops this is equivalent
        // to having bob wait to get the entry
        //
        // BUT! If bob is the one that gets the entry, then it caches it,
        // and the create_link succeeds. And we are trying to reproduce the create_link failing due to `Awaiting deps` error
        let r: Option<Record> = conductors[0]
            .call(
                &alice.zome(TestWasm::Link.coordinator_zome_name()),
                "test_entry_get",
                &action_hash,
            )
            .await;

        if r.is_some() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let _link: ActionHash = conductors[1]
        .call(
            &bob.zome(TestWasm::Link.coordinator_zome_name()),
            "test_entry_link",
            (action_hash.clone(), alice_pk.clone()),
        )
        .await;
}

pub mod must_get_agent_activity_saturation;
