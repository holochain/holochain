use hdk::prelude::{
    Entry, EntryDef, EntryDefIndex, EntryType, EntryVisibility, Op, ValidateCallbackResult,
};
use holo_hash::ActionHash;
use holochain::sweettest::{
    await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes,
};
use holochain_serialized_bytes::prelude::SerializedBytes;
use holochain_state::query::{CascadeTxnWrapper, Store};
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::CreateInput;
use serde::{Deserialize, Serialize};

// Test that warrants issuance can be disabled. This is useful for testing that
// warrants are propagated through publish and gossip.
//
// Alice creates invalid ops, Bob receives them but should not issue any warrant.
#[tokio::test(flavor = "multi_thread")]
async fn invalid_op_warrant_issuance_can_be_disabled() {
    holochain_trace::test_run();

    let zome_common = SweetInlineZomes::new(vec![], 0);
    let zome_without_validation = zome_common
        .clone()
        .integrity_function("validate", move |_api, _: Op| {
            Ok(ValidateCallbackResult::Valid)
        });
    // Any action is invalid, including genesis actions.
    let zome_with_validation = zome_common
        .clone()
        .integrity_function("validate", move |_api, _: Op| {
            Ok(ValidateCallbackResult::Invalid("nope".to_string()))
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
    // Disable warrants on Bob's conductor.
    let config_no_warranting = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|tc| tc.disable_warrant_issuance = true);
    let mut conductors =
        SweetConductorBatch::from_configs_rendezvous([config, config_no_warranting]).await;
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

    await_consistency(10, [&alice, &bob]).await.unwrap();

    // Bob must not have issued a warrant against Alice.
    // A warrant would have been created as part of app validating all of Alice's
    // ops, so once consistency is reached, the authored DB can be checked
    // for warrants.
    let alice_pubkey = alice.agent_pubkey().clone();
    conductors[1]
        .get_spaces()
        .get_all_authored_dbs(dna_hash)
        .unwrap()[0]
        .test_read(move |txn| {
            let store = CascadeTxnWrapper::from(txn);
            // TODO: The check_valid argument of get_warrants_for_agents should be removed once warrants are validated.
            let warrants = store.get_warrants_for_agent(&alice_pubkey, false).unwrap();
            assert!(warrants.is_empty());
        });
}

#[tokio::test(flavor = "multi_thread")]
async fn skip_self_validation_to_cause_warrant() {
    holochain_trace::test_run();

    let entry_def = EntryDef::default_from_id("any");
    let inline_zomes = SweetInlineZomes::new(vec![entry_def], 0)
        .function("create", move |api, _: ()| {
            #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
            struct S(String);

            let entry = Entry::app(S("a string".to_string()).try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .integrity_function("validate", move |_api, op: Op| {
            match op {
                Op::StoreRecord(record) => {
                    match record.record.action().entry_type() {
                        Some(EntryType::App(_)) => {
                            // Invalidates all app entries.
                            Ok(ValidateCallbackResult::Invalid("nope".to_string()))
                        }
                        _ => Ok(ValidateCallbackResult::Valid),
                    }
                }
                _ => Ok(ValidateCallbackResult::Valid),
            }
        });

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(inline_zomes).await;

    // Disable self-validation on Alice's conductor.
    let config_no_self_validation = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|tc| tc.disable_self_validation = true);
    let config = SweetConductorConfig::rendezvous(true);

    let mut conductors =
        SweetConductorBatch::from_configs_rendezvous([config_no_self_validation, config]).await;

    let (alice,) = conductors[0]
        .setup_app("test_app", [&dna_file])
        .await
        .unwrap()
        .into_tuple();
    let (bob,) = conductors[1]
        .setup_app("test_app", [&dna_file])
        .await
        .unwrap()
        .into_tuple();

    await_consistency(10, [&alice, &bob]).await.unwrap();

    // Now Alice creates some data that Bob will reject, causing Bob to issue a warrant against Alice.
    let _: ActionHash = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    // Should sync the data to Bob.
    await_consistency(10, [&alice, &bob]).await.unwrap();

    let alice_pubkey = alice.agent_pubkey().clone();
    let warrants = conductors[1]
        .get_spaces()
        .get_all_authored_dbs(dna_file.dna_hash())
        .unwrap()[0]
        .test_read({
            let alice_pubkey = alice_pubkey.clone();
            move |txn| {
                let store = CascadeTxnWrapper::from(txn);
                // TODO: The check_valid argument of get_warrants_for_agents should be removed once warrants are validated.
                store.get_warrants_for_agent(&alice_pubkey, false).unwrap()
            }
        });

    assert_eq!(1, warrants.len(), "Should have issued a warrant");
    assert_eq!(
        warrants[0].warrantee, alice_pubkey,
        "Warrant should be against Alice"
    );
}
