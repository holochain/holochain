use hdk::prelude::{
    CellId, ChainTopOrdering, CreateInput, EntryDef, EntryDefIndex, EntryVisibility, Op,
    SerializedBytes, ValidateCallbackResult,
};
use holo_hash::ActionHash;
use holochain::{
    prelude::InlineZomeSet,
    sweettest::{
        await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
        SweetInlineZomes,
    },
};
use holochain_state::query::{CascadeTxnWrapper, Store};
use holochain_zome_types::Entry;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Alice creates an invalid op and publishes it to Bob. Bob issues a warrant and
// blocks Alice.
#[tokio::test(flavor = "multi_thread")]
async fn warranted_agent_is_blocked() {
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

    let config = SweetConductorConfig::standard();
    let config_without_publish = config
        .clone()
        .tune_network_config(|nc| nc.disable_publish = true)
        .tune_conductor(|tc| tc.min_publish_interval = Some(Duration::from_secs(10)));
    let mut conductors = SweetConductorBatch::from_configs_rendezvous([
        config_without_publish.clone(),
        config_without_publish,
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

    // Let all agents sync.
    await_consistency(10, [&alice, &bob]).await.unwrap();

    // Alice creates an invalid action.
    let _: ActionHash = conductors[0]
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            AppString("entry1".into()),
        )
        .await;

    await_consistency(10, [&alice, &bob]).await.unwrap();

    // The warrant against Alice and the warrant op should have been written to Bob's authored database.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let alice_pubkey = alice.agent_pubkey().clone();
            let warrants = conductors[1]
                .get_spaces()
                .get_all_authored_dbs(dna_hash)
                .unwrap()[0]
                .test_read(move |txn| {
                    let store = CascadeTxnWrapper::from(txn);
                    store.get_warrants_for_agent(&alice_pubkey, false).unwrap()
                });

            if warrants.len() == 1 && warrants[0].warrant().warrantee == *alice.agent_pubkey() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Check that Alice is blocked by Bob.
    let target = hdk::prelude::BlockTargetId::Cell(CellId::new(
        dna_hash.clone(),
        alice.agent_pubkey().clone(),
    ));
    assert!(conductors[1]
        .holochain_p2p()
        .is_blocked(target)
        .await
        .unwrap());
}

// Test that warrants are gossiped and received.
// Alice, Bob and Carol start a network and sync. Carol goes offline.
// Alice creates an invalid op, Bob receives it and issues a warrant.
// Carol has warrant issuance disabled and receives the warrant from Bob
// via gossip after she comes back online.
// Publish is disabled for this test.
#[tokio::test(flavor = "multi_thread")]
async fn warrant_is_gossiped() {
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

    let config =
        SweetConductorConfig::rendezvous(true).tune_network_config(|nc| nc.disable_publish = true);
    // Disable warrants on Carol's conductor, so that she doesn't issue warrants herself
    // but receives them from Bob.
    let config_no_warranting = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|tc| tc.disable_warrant_issuance = true)
        .tune_network_config(|nc| nc.disable_publish = true);
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

    await_consistency(10, [&alice, &bob, &carol]).await.unwrap();

    // Shutdown Carol's conductor.
    conductors[2].shutdown().await;

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

    // Shutdown Alice and startup Carol.
    conductors[0].shutdown().await;
    conductors[2].startup(false).await;

    // Carol should receive the warrant against Alice.
    // The warrant and the warrant op should have been written to the DHT database,
    // as well as the invalid ops.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let alice_pubkey = alice.agent_pubkey().clone();
            let invalid_ops = conductors[2]
                .get_invalid_integrated_ops(&conductors[2].get_dht_db(dna_hash).unwrap())
                .await
                .unwrap();
            if invalid_ops.len() == 3 {
                let warrants = conductors[2]
                    .get_spaces()
                    .dht_db(dna_hash)
                    .unwrap()
                    .test_read(move |txn| {
                        let store = CascadeTxnWrapper::from(txn);
                        // TODO: check_valid here should be removed once warrants are validated.
                        store.get_warrants_for_agent(&alice_pubkey, false).unwrap()
                    });
                if warrants.len() == 1 {
                    assert_eq!(warrants[0].warrant().warrantee, *alice.agent_pubkey());
                    // Make sure that Bob authored the warrant and it's not been authored by Carol.
                    assert_eq!(warrants[0].warrant().author, *bob.agent_pubkey());
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
}
