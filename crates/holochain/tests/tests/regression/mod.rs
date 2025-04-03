use holo_hash::ActionHash;
#[cfg(not(feature = "wasmer_wamr"))]
use holochain::conductor::conductor::WASM_CACHE;
use holochain::sweettest::*;
use holochain_wasm_test_utils::TestWasm;

// Make sure the wasm cache at least creates files.
// This is not run with the `wasmer_wamr` feature flag,
// as the cache is not used.
#[tokio::test(flavor = "multi_thread")]
#[cfg(not(feature = "wasmer_wamr"))]
async fn wasm_disk_cache() {
    holochain_trace::test_run();
    let mut conductor =
        SweetConductor::from_config(SweetConductorConfig::standard().no_dpki()).await;

    let mut cache_dir = conductor.db_path().to_owned();
    cache_dir.push(WASM_CACHE);

    let mut read = tokio::fs::read_dir(&cache_dir).await.unwrap();
    assert!(read.next_entry().await.unwrap().is_none());

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ValidateRejectAppTypes, TestWasm::Crd])
            .await;
    let agent = SweetAgents::alice();

    let (cell,) = conductor
        .setup_app_for_agent("app", agent, &[dna_file])
        .await
        .unwrap()
        .into_tuple();

    let _created: ActionHash = conductor
        .call(
            &cell.zome(TestWasm::Crd.coordinator_zome_name()),
            "create",
            (),
        )
        .await;

    let mut read = tokio::fs::read_dir(&cache_dir).await.unwrap();
    assert!(read.next_entry().await.unwrap().is_some());
}

// Intended to keep https://github.com/holochain/holochain/issues/2868 fixed.
#[tokio::test(flavor = "multi_thread")]
async fn zome_with_no_entry_types_does_not_prevent_deletes() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ValidateRejectAppTypes, TestWasm::Crd])
            .await;

    let (cell,) = conductor
        .setup_app("app", &[dna_file])
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

    let (cell,) = conductor
        .setup_app("app", &[dna_file])
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
#[ignore = "flaky"]
async fn zero_arc_can_link_to_uncached_base() {
    use hdk::prelude::*;

    holochain_trace::test_run();

    let empty_arc_conductor_config = SweetConductorConfig::rendezvous(false)
        .no_dpki_mustfix()
        .tune_network_config(|nc| nc.target_arc_factor = 0);

    let other_config = SweetConductorConfig::rendezvous(false)
        .no_dpki_mustfix()
        .tune_network_config(|nc| {
            // Expecting Alice and Bob to update their storage arcs, so make them
            // publish that change sooner.
            nc.advanced
                .as_mut()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(
                    "coreSpace".to_string(),
                    serde_json::json!({
                        "reSignFreqMs": 500,
                        "reSignExpireTimeMs": (19.9 * 60.0 * 1000.0) as u32,
                    }),
                );
        });
    let mut conductors = SweetConductorBatch::from_configs_rendezvous(vec![
        // Need a pair of conductors so that they can agree that their initial view of the DHT is
        // complete. With just another 0 arc node, alice can never gossip and cannot discover that
        // they're okay to declare a full arc.
        other_config.clone(),
        other_config,
        empty_arc_conductor_config,
    ])
    .await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
        TestWasm::ValidateRejectAppTypes,
        TestWasm::Link,
    ])
    .await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let ((alice,), (bob,), (carol_empty_arc,)) = apps.into_tuples();

    // For initial peer discovery (bootstrap disabled in this test)
    conductors.exchange_peer_info().await;

    conductors[0]
        .require_initial_gossip_activity_for_cell(&alice, 1, std::time::Duration::from_secs(30))
        .await
        .unwrap();

    conductors[1]
        .require_initial_gossip_activity_for_cell(&bob, 1, std::time::Duration::from_secs(30))
        .await
        .unwrap();

    // Wait for Alice to declare a full arc.
    tokio::time::timeout(std::time::Duration::from_secs(30), {
        let c0 = conductors[0].clone();
        let alice = alice.clone();
        async move {
            loop {
                let alice = c0
                    .holochain_p2p()
                    .peer_store(alice.dna_hash().clone())
                    .await
                    .unwrap()
                    .get(alice.agent_pubkey().to_k2_agent())
                    .await
                    .unwrap();

                if let Some(a) = alice {
                    if a.storage_arc == kitsune2_api::DhtArc::FULL {
                        break;
                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
    .await
    .expect("Alice did not declare a full arc");

    // Now we want to update agent infos where Alice and Bob should have declared full arc.
    conductors.exchange_peer_info().await;

    let alice_pk = alice.cell_id().agent_pubkey().clone();

    println!("@!@!@ alice_pk: {alice_pk:?}");

    let action_hash: ActionHash = conductors[0]
        .call(
            &alice.zome(TestWasm::Link.coordinator_zome_name()),
            "test_entry_create",
            (),
        )
        .await;

    println!("@!@!@ -- must_get_valid_record --");
    println!("@!@!@ action_hash: {action_hash:?}");

    // Carol is linking to Alice's action hash, but doesn't have it locally
    // so the must_get_valid_record in validation will have to do a network get.
    let link_hash: ActionHash = conductors[2]
        .call(
            &carol_empty_arc.zome(TestWasm::Link.coordinator_zome_name()),
            "link_validation_calls_must_get_valid_record",
            (action_hash.clone(), alice_pk.clone()),
        )
        .await;

    println!("@!@!@ link_hash: {link_hash:?}");

    let action_hash: ActionHash = conductors[0]
        .call(
            &alice.zome(TestWasm::Link.coordinator_zome_name()),
            "test_entry_create",
            (),
        )
        .await;

    println!("@!@!@ -- must_get_action / must_get_entry --");
    println!("@!@!@ action_hash: {action_hash:?}");

    // Carol is linking to Alice's action hash, but doesn't have it locally
    // so the must_get_entry/must_get_action in validation will have to do a network get.
    let link_hash: ActionHash = conductors[2]
        .call(
            &carol_empty_arc.zome(TestWasm::Link.coordinator_zome_name()),
            "link_validation_calls_must_get_action_then_entry",
            (action_hash.clone(), alice_pk.clone()),
        )
        .await;

    println!("@!@!@ link_hash: {link_hash:?}");

    let action_hash: ActionHash = conductors[0]
        .call(
            &alice.zome(TestWasm::Link.coordinator_zome_name()),
            "test_entry_create",
            (),
        )
        .await;

    println!("@!@!@ -- must_get_agent_activity --");
    println!("@!@!@ action_hash: {action_hash:?}");

    // Carol is linking to Alice's action hash, but doesn't have it locally
    // so the must_get_agent_activity in validation will have to do a network get.
    let link_hash: ActionHash = conductors[2]
        .call(
            &carol_empty_arc.zome(TestWasm::Link.coordinator_zome_name()),
            "link_validation_calls_must_get_agent_activity",
            (action_hash.clone(), alice_pk.clone()),
        )
        .await;

    println!("@!@!@ link_hash: {link_hash:?}");
}

pub mod must_get_agent_activity_saturation;
