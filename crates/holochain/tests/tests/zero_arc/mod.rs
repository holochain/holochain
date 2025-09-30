use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use schemars::_private::NoSerialize;
use hdk::prelude::{Deserialize, Entry, EntryDef, EntryDefIndex, EntryHashed, EntryType, EntryVisibility, MustGetActionInput, MustGetEntryInput, Op, Record, Serialize, ValidateCallbackResult};
use holo_hash::{ActionHash, EntryHash};
use holochain::core::ValidationOutcome;
use holochain::prelude::{SerializedBytes, SignedActionHashed};
use holochain::sweettest::{
    SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes,
};
use holochain_state::prelude::MustGetValidRecordInput;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::{CreateInput, GetInput};
use holochain_zome_types::prelude::{Details, GetOptions};

/// Test that zero arc nodes can use the various get host functions to get missing records, actions
/// and entries from authorities.
#[tokio::test(flavor = "multi_thread")]
async fn get_missing_from_coordinator() {
    holochain_trace::test_run();

    let entry_def = EntryDef::default_from_id("entry");
    let zomes = SweetInlineZomes::new(vec![entry_def], 0)
        .function("create",
            move |api, _: ()| {
                let entry = Entry::app(().try_into().unwrap()).unwrap();
                let hash = api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                    EntryVisibility::Public,
                    entry,
                    ChainTopOrdering::default(),
                ))?;
                let details = api.get_details(vec![GetInput::new(
                    hash.clone().into(),
                    GetOptions::local(),
                )])?;
                let entry_hash = match details[0].as_ref().unwrap() {
                    Details::Record(record_details) => {
                        record_details.record.action().entry_hash().unwrap().clone()
                    }
                    _ => panic!("Expected record details"),
                };

                Ok((hash, entry_hash))
            }
        )
        .function("get", move |api, hash: ActionHash| {
            let records = api.get(vec![GetInput::new(hash.into(), GetOptions::network())])?;
            Ok(records)
        })
        .function("get_details", move |api, hash: ActionHash| {
            let details =
                api.get_details(vec![GetInput::new(hash.into(), GetOptions::network())])?;
            Ok(details)
        })
        .function("must_get_valid_record", move |api, hash: ActionHash| {
            let record = api.must_get_valid_record(MustGetValidRecordInput::new(hash))?;
            Ok(record)
        })
        .function("must_get_action",
            move |api, hash: ActionHash| {
                let action = api.must_get_action(MustGetActionInput::new(hash))?;
                Ok(action)
            }
        )
        .function("must_get_entry", move |api, hash: EntryHash| {
            let action = api.must_get_entry(MustGetEntryInput::new(hash))?;
            Ok(action)
        })
        .0;

    let (test_dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    // Standard config with target arc factor 1
    let standard_config = SweetConductorConfig::rendezvous(false).tune_network_config(|nc| {
        nc.disable_gossip = true;
        nc.disable_publish = true;
    });
    // Standard config with target arc factor 0
    let empty_arc_conductor_config =
        SweetConductorConfig::rendezvous(false).tune_network_config(|nc| {
            nc.disable_gossip = true;
            nc.disable_publish = true;
            nc.target_arc_factor = 0;
        });
    let mut conductors =
        SweetConductorBatch::from_configs_rendezvous([standard_config, empty_arc_conductor_config])
            .await;

    let apps = conductors.setup_app("", [&test_dna]).await.unwrap();
    let ((alice, ), (bob, )) = apps.into_tuples();

    // Alice creates several entries
    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    let (get_hash, _): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;
    let (get_details_hash, _): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;
    let (must_get_valid_record_hash, _): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;
    let (must_get_action_hash, _): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;
    let (_, entry_hash): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;

    // Ensure Bob cannot see Alice's entries
    let bob_zome = bob.zome(SweetInlineZomes::COORDINATOR);
    let records: Vec<Option<Record>> = conductors[1].call(&bob_zome, "get", get_hash.clone()).await;
    assert!(
        records.into_iter().all(|r| r.is_none()),
        "Expected Bob to not be able to get Alice's record"
    );
    let details: Vec<Option<Details>> = conductors[1]
        .call(&bob_zome, "get_details", get_details_hash.clone())
        .await;
    assert!(
        details.into_iter().all(|d| d.is_none()),
        "Expected Bob to not be able to get_details of Alice's record"
    );
    conductors[1]
        .call_fallible::<_, Record>(
            &bob_zome,
            "must_get_valid_record",
            must_get_valid_record_hash.clone(),
        )
        .await
        .expect_err("Expected Bob to not be able to must_get_valid_record of Alice's record");
    conductors[1]
        .call_fallible::<_, SignedActionHashed>(
            &bob_zome,
            "must_get_action",
            must_get_action_hash.clone(),
        )
        .await
        .expect_err("Expected Bob to not be able to must_get_action of Alice's record");
    conductors[1]
        .call_fallible::<_, EntryHashed>(&bob_zome, "must_get_entry", entry_hash.clone())
        .await
        .expect_err("Expected Bob to not be able to must_get_entry of Alice's record");

    // Simulate Alice reaching a full storage arc
    conductors[0]
        .declare_full_storage_arcs(test_dna.dna_hash())
        .await;
    // and ensure Bob knows about Alice's full arc.
    conductors.exchange_peer_info().await;

    // Now Bob should be able to get Alice's entry
    let records: Vec<Option<Record>> = conductors[1].call(&bob_zome, "get", get_hash).await;
    assert_eq!(
        records.len(),
        1,
        "Expected Bob to be able to get Alice's record"
    );
    assert!(
        records.into_iter().all(|r| r.is_some()),
        "Expected Bob to be able to get Alice's record"
    );

    let details: Vec<Option<Details>> = conductors[1]
        .call(&bob_zome, "get_details", get_details_hash)
        .await;
    assert_eq!(
        details.len(),
        1,
        "Expected Bob to be able to get_details of Alice's record"
    );
    assert!(
        details.into_iter().all(|d| d.is_some()),
        "Expected Bob to be able to get_details of Alice's record"
    );

    let record = conductors[1]
        .call::<_, Record>(
            &bob_zome,
            "must_get_valid_record",
            must_get_valid_record_hash.clone(),
        )
        .await;
    assert_eq!(must_get_valid_record_hash, record.action_address().clone());

    let action = conductors[1]
        .call::<_, SignedActionHashed>(&bob_zome, "must_get_action", must_get_action_hash.clone())
        .await;
    assert_eq!(must_get_action_hash, action.as_hash().clone());

    let entry = conductors[1]
        .call::<_, EntryHashed>(&bob_zome, "must_get_entry", entry_hash.clone())
        .await;
    assert_eq!(entry_hash, entry.hash);
}

/// Test that zero arc nodes can retrieve missing data during self validation.
#[tokio::test(flavor = "multi_thread")]
async fn self_validation_get_missing() {
    holochain_trace::test_run();

    #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
    struct TestDepEntry {
        action_hash: ActionHash,
        num: u8,
    }

    let invoked_must_get_valid_record = Arc::new(AtomicBool::new(false));

    let entry_def = EntryDef::default_from_id("entry");
    let dep_entry_def = EntryDef::default_from_id("dep_entry");
    let zomes = SweetInlineZomes::new(vec![entry_def, dep_entry_def], 0)
        // A "create" function that can be used to create entries on a full arc node
        .function("create",
                  move |api, _: ()| {
                      let entry = Entry::app(().try_into().unwrap()).unwrap();
                      let hash = api.create(CreateInput::new(
                          InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                          EntryVisibility::Public,
                          entry,
                          ChainTopOrdering::default(),
                      ))?;

                      Ok(hash)
                  },
        )
        // A "createWithDep" function that creates an entry that depends on another entry
        .function("create_with_dep", async move |api, (num, dep_hash): (u8, ActionHash)| {
            let data = TestDepEntry {
                action_hash: dep_hash,
                num,
            };
            let app_bytes: Vec<u8> = data.try_into().unwrap();
            let entry = Entry::app(SerializedBytes::from(UnsafeBytes::from(app_bytes))).unwrap();
            api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(1)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;

            Ok(())
        })
        .integrity_function("validate", {
            let invoked_must_get_valid_record = invoked_must_get_valid_record.clone();
            move |api, op: Op| {
                match op {
                    Op::StoreRecord(store_record) => {
                        match store_record.record.action().entry_type() {
                            Some(EntryType::App(app_entry_def)) => {
                                if app_entry_def.entry_index.0 != 1 {
                                    return Ok(ValidateCallbackResult::Valid);
                                }
                            }
                            _ => {
                                return Ok(ValidateCallbackResult::Valid);
                            }
                        }

                        let app_entry: Option<TestDepEntry> = store_record.record.entry.to_app_option().unwrap();
                        let app_entry = app_entry.unwrap();

                        match app_entry.num {
                            0 => {
                                let _record = api.must_get_valid_record(MustGetValidRecordInput::new(
                                    app_entry.action_hash.clone(),
                                ))?;
                                invoked_must_get_valid_record.store(true, Ordering::Relaxed);
                                Ok(ValidateCallbackResult::Valid)
                            }
                            _ => {
                                Ok(ValidateCallbackResult::Invalid(
                                    "Unhandled value for num".to_string()
                                ))
                            }
                        }

                    }
                    _ => Ok(ValidateCallbackResult::Valid)
                }
            }
        })
            .0;
}
