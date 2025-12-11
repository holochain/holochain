use holo_hash::ActionHash;
use holochain::sweettest::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::GetValidationReceiptsInput;
use holochain_zome_types::validate::ValidationReceiptSet;
use {
    hdk::prelude::{
        ChainTopOrdering, CreateInput, EntryDef, EntryDefIndex, EntryVisibility, Op,
        SerializedBytes, ValidateCallbackResult,
    },
    holochain::prelude::InlineZomeSet,
    holochain_state::query::{CascadeTxnWrapper, Store},
    holochain_zome_types::Entry,
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

/// Verifies that publishing terminates naturally when enough validation receipts are received.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(any(target_os = "linux", all(target_os = "macos", feature = "wasmer_sys"))),
    ignore = "flaky on macos+wasmer_wamr and windows"
)]
async fn publish_terminates_after_receiving_required_validation_receipts() {
    use holochain::test_utils::retry_fn_until_timeout;

    holochain_trace::test_run();

    // Need DEFAULT_RECEIPT_BUNDLE_SIZE peers to send validation receipts back
    const NUM_CONDUCTORS: usize =
        holochain::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE as usize
            + 1;

    let config = SweetConductorConfig::rendezvous(true).tune_conductor(|cc| {
        cc.min_publish_interval = Some(Duration::from_secs(5));
        cc.publish_trigger_interval = Some(Duration::from_secs(5))
    });
    let mut conductors = SweetConductorBatch::from_config_rendezvous(NUM_CONDUCTORS, config).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let alice_cell = apps.cells_flattened().into_iter().next().unwrap();
    let alice_zome = alice_cell.zome(TestWasm::Create);

    for c in conductors.iter() {
        c.declare_full_storage_arcs(alice_cell.dna_hash()).await;
    }

    conductors.exchange_peer_info().await;

    // write an action
    let action_hash: ActionHash = conductors[0].call(&alice_zome, "create_entry", ()).await;

    // wait for validation receipts
    retry_fn_until_timeout(
        || async {
            // check for complete count of our receipts on the
            // millisecond level

            // Get the validation receipts to check that they
            // are all complete
            let receipt_sets: Vec<ValidationReceiptSet> = conductors[0]
                .call(
                    &alice_zome,
                    "get_validation_receipts",
                    GetValidationReceiptsInput::new(action_hash.clone()),
                )
                .await;

            let receipt_sets_len = receipt_sets.len() == 3;
            let receipt_sets_complete = receipt_sets.iter().all(|r| r.receipts_complete);
            let agent_activity_receipt_set = match receipt_sets
                .into_iter()
                .find(|r| r.op_type == "RegisterAgentActivity")
            {
                None => 0,
                Some(r) => r.receipts.len(),
            }
                == holochain::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE
                    as usize;

            receipt_sets_len && receipt_sets_complete && agent_activity_receipt_set
        },
        Some(60_000),
        None,
    )
    .await
    .unwrap();
}

// Test that warrants are published and received.
// Alice creates an invalid op, Bob receives it and issues a warrant.
// Carol has warrant issuance disabled and receives the warrant from Bob
// as he publishes it.
#[tokio::test(flavor = "multi_thread")]
async fn warrant_is_published() {
    holochain_trace::test_run();

    #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
    struct AppString(String);

    let string_entry_def = EntryDef::default_from_id("string");
    let zome_common = SweetInlineZomes::new(vec![string_entry_def], 0).function(
        "create_string",
        move |api, s: AppString| {
            let entry = Entry::app(s.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        },
    );

    let zome_without_validation = zome_common
        .clone()
        .integrity_function("validate", move |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        });
    // Any action after the genesis actions is invalid.
    let zome_with_validation =
        zome_common
            .clone()
            .integrity_function("validate", move |_api, op: Op| {
                if op.action_seq() > 3 {
                    Ok(ValidateCallbackResult::Invalid("nope".to_string()))
                } else {
                    Ok(ValidateCallbackResult::Valid)
                }
            });

    let network_seed = "seed".to_string();

    let (dna_without_validation, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_without_validation).await;
    let (dna_with_validation, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_with_validation).await;
    assert_eq!(
        dna_without_validation.dna_hash(),
        dna_with_validation.dna_hash()
    );
    let dna_hash = dna_without_validation.dna_hash();

    let config = SweetConductorConfig::rendezvous(true);
    // Disable warrants on Carol's conductor, so that she doesn't issue warrants herself
    // but receives them from Bob.
    let config_no_warranting = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|tc| tc.disable_warrant_issuance = true);
    let mut conductors = SweetConductorBatch::from_configs_rendezvous([
        config.clone(),
        config,
        config_no_warranting,
    ])
    .await;
    let (alice,) = conductors[0]
        .setup_app("test_app", [&dna_without_validation])
        .await
        .unwrap()
        .into_tuple();
    let (bob,) = conductors[1]
        .setup_app("test_app", [&dna_with_validation])
        .await
        .unwrap()
        .into_tuple();
    let (carol,) = conductors[2]
        .setup_app("test_app", [&dna_with_validation])
        .await
        .unwrap()
        .into_tuple();

    println!("AGENTS");
    println!(
        "0 alice {} url {:?}",
        alice.agent_pubkey(),
        conductors[0]
            .dump_network_stats()
            .await
            .unwrap()
            .transport_stats
            .peer_urls[0]
    );
    println!(
        "1 bob   {} url {:?}",
        bob.agent_pubkey(),
        conductors[1]
            .dump_network_stats()
            .await
            .unwrap()
            .transport_stats
            .peer_urls[0]
    );
    println!(
        "2 carol {} url {:?}",
        carol.agent_pubkey(),
        conductors[2]
            .dump_network_stats()
            .await
            .unwrap()
            .transport_stats
            .peer_urls[0]
    );

    await_consistency(10, [&alice, &bob, &carol]).await.unwrap();

    // Alice creates an invalid action.
    let _: ActionHash = conductors[0]
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            AppString("entry1".into()),
        )
        .await;

    await_consistency(10, [&alice, &bob]).await.unwrap();

    // Bob should have issued a warrant against Alice.

    // Carol should receive the warrant against Alice.
    // The warrant and the warrant op should have been written to the authored databases.
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let alice_pubkey = alice.agent_pubkey().clone();
            let warrants = conductors[2]
                .get_spaces()
                .dht_db(dna_hash)
                .unwrap()
                .test_read(move |txn| {
                    let store = CascadeTxnWrapper::from(txn);
                    store.get_warrants_for_agent(&alice_pubkey, true).unwrap()
                });

            if warrants.len() == 1 {
                assert_eq!(warrants[0].warrant().warrantee, *alice.agent_pubkey());
                // Make sure that Bob authored the warrant and it's not been authored by Carol.
                assert_eq!(warrants[0].warrant().author, *bob.agent_pubkey());
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
}
