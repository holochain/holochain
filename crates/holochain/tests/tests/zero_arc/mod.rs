use hdk::prelude::{
    Deserialize, Entry, EntryDef, EntryDefIndex, EntryHashed, EntryType, EntryVisibility,
    LinkTypeFilter, MustGetActionInput, MustGetEntryInput, Op, Record, Serialize,
    ValidateCallbackResult,
};
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::{ActionHash, EntryHash};
use holochain::prelude::{SerializedBytes, SignedActionHashed};
use holochain::sweettest::{
    SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes,
};
use holochain::test_utils::retry_fn_until_timeout;
use holochain_state::prelude::MustGetValidRecordInput;
use holochain_trace::test_run;
use holochain_types::fixt::AppEntryBytesFixturator;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::{CreateInput, GetInput, UpdateInput};
use holochain_zome_types::link::{CreateLinkInput, DeleteLinkInput, GetLinksInput, Link};
use holochain_zome_types::metadata::RecordDetails;
use holochain_zome_types::prelude::{BoxApi, Details, GetOptions, LinkQuery};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Test that zero arc nodes can use the various get host functions to get missing records, actions
/// and entries from authorities.
#[tokio::test(flavor = "multi_thread")]
async fn get_missing_from_coordinator() {
    holochain_trace::test_run();

    let entry_def = EntryDef::default_from_id("entry");
    let zomes = SweetInlineZomes::new(vec![entry_def], 0)
        .function("create", move |api, _: ()| {
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
        })
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
        .function("must_get_action", move |api, hash: ActionHash| {
            let action = api.must_get_action(MustGetActionInput::new(hash))?;
            Ok(action)
        })
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
    let ((alice,), (bob,)) = apps.into_tuples();

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
        entry_hash: EntryHash,
        num: u8,
    }

    let invoked_must_get_valid_record = Arc::new(AtomicBool::new(false));
    let invoked_must_get_action = Arc::new(AtomicBool::new(false));
    let invoked_must_get_entry = Arc::new(AtomicBool::new(false));

    let entry_def = EntryDef::default_from_id("entry");
    let dep_entry_def = EntryDef::default_from_id("dep_entry");
    let zomes = SweetInlineZomes::new(vec![entry_def, dep_entry_def], 0)
        // A "create" function that can be used to create entries on a full arc node
        .function("create", move |api, _: ()| {
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
        })
        // A "create_with_dep" function that creates an entry that depends on another entry
        .function(
            "create_with_dep",
            move |api, (num, dep_hash, dep_entry_hash): (u8, ActionHash, EntryHash)| {
                let data = TestDepEntry {
                    action_hash: dep_hash,
                    entry_hash: dep_entry_hash,
                    num,
                };
                let app_bytes: holochain_serialized_bytes::SerializedBytes =
                    data.try_into().unwrap();
                let entry = Entry::app(app_bytes).unwrap();
                api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, EntryDefIndex(1)),
                    EntryVisibility::Public,
                    entry,
                    ChainTopOrdering::default(),
                ))?;
                Ok(())
            },
        )
        .integrity_function("validate", {
            let invoked_must_get_valid_record = invoked_must_get_valid_record.clone();
            let invoked_must_get_action = invoked_must_get_action.clone();
            let invoked_must_get_entry = invoked_must_get_entry.clone();
            move |api, op: Op| match op {
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
                    let app_entry: Option<TestDepEntry> =
                        store_record.record.entry.to_app_option().unwrap();
                    let app_entry = app_entry.unwrap();
                    match app_entry.num {
                        0 => {
                            let _record = api.must_get_valid_record(
                                MustGetValidRecordInput::new(app_entry.action_hash.clone()),
                            )?;
                            invoked_must_get_valid_record.store(true, Ordering::Relaxed);
                            Ok(ValidateCallbackResult::Valid)
                        }
                        1 => {
                            let _action = api.must_get_action(MustGetActionInput::new(
                                app_entry.action_hash.clone(),
                            ))?;
                            invoked_must_get_action.store(true, Ordering::Relaxed);
                            Ok(ValidateCallbackResult::Valid)
                        }
                        2 => {
                            let _entry = api.must_get_entry(MustGetEntryInput::new(
                                app_entry.entry_hash.clone(),
                            ))?;
                            invoked_must_get_entry.store(true, Ordering::Relaxed);
                            Ok(ValidateCallbackResult::Valid)
                        }
                        _ => Ok(ValidateCallbackResult::Invalid(
                            "Unhandled value for num".to_string(),
                        )),
                    }
                }
                _ => Ok(ValidateCallbackResult::Valid),
            }
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
    let ((alice,), (bob,)) = apps.into_tuples();

    // Alice creates entries and gets their hashes
    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    let (action_hash_must_get_record, entry_hash_must_get_record): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;
    let (action_hash_must_get_action, entry_hash_must_get_action): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;
    let (action_hash_must_get_entry, entry_hash_must_get_entry): (ActionHash, EntryHash) =
        conductors[0].call(&alice_zome, "create", ()).await;

    // Simulate Alice reaching a full storage arc
    conductors[0]
        .declare_full_storage_arcs(test_dna.dna_hash())
        .await;
    // and ensure Bob knows about Alice's full arc.
    conductors.exchange_peer_info().await;

    let bob_zome = bob.zome(SweetInlineZomes::COORDINATOR);

    // Bob creates dependent entries to trigger each must_get_* operation
    conductors[1]
        .call::<_, ()>(
            &bob_zome,
            "create_with_dep",
            (
                0u8,
                action_hash_must_get_record.clone(),
                entry_hash_must_get_record.clone(),
            ),
        )
        .await;
    conductors[1]
        .call::<_, ()>(
            &bob_zome,
            "create_with_dep",
            (
                1u8,
                action_hash_must_get_action.clone(),
                entry_hash_must_get_action.clone(),
            ),
        )
        .await;
    conductors[1]
        .call::<_, ()>(
            &bob_zome,
            "create_with_dep",
            (2u8, action_hash_must_get_entry, entry_hash_must_get_entry),
        )
        .await;

    // Assert that each must_get_* was invoked
    assert!(
        invoked_must_get_valid_record.load(Ordering::Relaxed),
        "must_get_valid_record was not invoked"
    );
    assert!(
        invoked_must_get_action.load(Ordering::Relaxed),
        "must_get_action was not invoked"
    );
    assert!(
        invoked_must_get_entry.load(Ordering::Relaxed),
        "must_get_entry was not invoked"
    );
}

/// This test ensures that a 0-arc node can discover updates to an entry
/// made by any other node, using get_details.
#[tokio::test(flavor = "multi_thread")]
async fn zero_arc_get_details_discover_updates() {
    holochain_trace::test_run();

    let entry_def = EntryDef::default_from_id("entry");
    let zomes = SweetInlineZomes::new(vec![entry_def], 0)
        // Create function to build the initial entry
        .function("create", move |api: BoxApi, _: ()| {
            let entry_bytes = fixt::fixt!(AppEntryBytes);
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                Entry::App(entry_bytes),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        // Update function to create an update to an existing entry
        .function("update", move |api, original_action_hash: ActionHash| {
            let entry_bytes = fixt::fixt!(AppEntryBytes);
            let hash = api.update(UpdateInput::new(
                original_action_hash,
                Entry::App(entry_bytes),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        // Wrapper to call get_details
        .function("get_details", move |api, hash: ActionHash| {
            let details =
                api.get_details(vec![GetInput::new(hash.into(), GetOptions::network())])?;
            Ok(details)
        });

    let (test_dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    // Standard config
    let standard_config = SweetConductorConfig::rendezvous(false);
    // Standard config with target arc factor 0
    let zero_arc_conductor_config =
        SweetConductorConfig::rendezvous(false).tune_network_config(|nc| {
            nc.target_arc_factor = 0;
        });

    let mut conductors =
        SweetConductorBatch::from_configs_rendezvous([standard_config, zero_arc_conductor_config])
            .await;

    let ((alice,), (bob,)) = conductors
        .setup_app("test", [&test_dna])
        .await
        .unwrap()
        .into_tuples();

    // Alice declares full storage arcs
    conductors[0]
        .declare_full_storage_arcs(test_dna.dna_hash())
        .await;
    // and ensure Bob knows about Alice's full arc.
    conductors.exchange_peer_info().await;

    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    let bob_zome = bob.zome(SweetInlineZomes::COORDINATOR);

    // Alice creates a record
    let entry_action_hash: ActionHash = conductors[0].call(&alice_zome, "create", ()).await;

    // Wait for Bob to discover the entry via get_details
    retry_fn_until_timeout(
        || async {
            let details: Vec<Option<Details>> = conductors[1]
                .call(&bob_zome, "get_details", entry_action_hash.clone())
                .await;

            if !details.is_empty()
                && details[0].is_some()
                && matches!(details[0].as_ref().unwrap(), Details::Record(RecordDetails {
            updates,
            deletes,
            ..
        }) if updates.is_empty() && deletes.is_empty())
            {
                return true;
            }

            false
        },
        None,
        None,
    )
    .await
    .unwrap();

    // Alice updates the record
    let updated_action_hash: ActionHash = conductors[0]
        .call(&alice_zome, "update", entry_action_hash.clone())
        .await;

    // Wait for Bob to discover the update via get_details
    retry_fn_until_timeout(
        || async {
            let details: Vec<Option<Details>> = conductors[1]
                .call(&bob_zome, "get_details", entry_action_hash.clone())
                .await;

            if let Details::Record(RecordDetails { updates, .. }) = details[0].as_ref().unwrap() {
                updates
                    .iter()
                    .any(|update| update.as_hash() == &updated_action_hash)
            } else {
                false
            }
        },
        None,
        None,
    )
    .await
    .unwrap();
}

/// Tests that after deleting a link, later calls to get_links will not return the deleted link.
#[tokio::test(flavor = "multi_thread")]
async fn zero_arc_delete_link_get_links() {
    test_run();

    let entry_def = EntryDef::default_from_id("entry");
    let zomes = SweetInlineZomes::new(vec![entry_def], 1)
        .function("create_link", move |api, base: ActionHash| {
            let hash = api.create_link(CreateLinkInput::new(
                base.into(),
                fixt::fixt!(ActionHash).into(),
                0.into(),
                0.into(),
                vec![].into(),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("delete_link", move |api, link_hash: ActionHash| {
            let _ = api.delete_link(DeleteLinkInput::new(
                link_hash,
                GetOptions::default(),
                ChainTopOrdering::default(),
            ))?;
            Ok(())
        })
        .function("get_links", move |api, base: ActionHash| {
            let links = api.get_links(vec![GetLinksInput::from_query(
                LinkQuery::new(base, LinkTypeFilter::single_type(0.into(), 0.into())),
                GetOptions::default(),
            )])?;
            Ok(links.into_iter().flatten().collect::<Vec<_>>())
        });

    let base = fixt::fixt!(ActionHash);

    let (test_dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let standard_conductor_config = SweetConductorConfig::rendezvous(false);
    let zero_arc_conductor_config =
        SweetConductorConfig::rendezvous(false).tune_network_config(|nc| {
            nc.target_arc_factor = 0;
        });

    let mut conductors = SweetConductorBatch::from_configs_rendezvous([
        standard_conductor_config,
        zero_arc_conductor_config,
    ])
    .await;

    let apps = conductors.setup_app("test_app", [&test_dna]).await.unwrap();

    let ((alice,), (bob,)) = apps.into_tuples();

    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    let bob_zome = bob.zome(SweetInlineZomes::COORDINATOR);

    // Simulate Alice reaching a full storage arc
    conductors[0]
        .declare_full_storage_arcs(test_dna.dna_hash())
        .await;

    // and ensure Bob knows about Alice's full arc.
    conductors.exchange_peer_info().await;

    // Alice creates a link
    let link_hash: ActionHash = conductors[0]
        .call(&alice_zome, "create_link", base.clone())
        .await;

    // Wait for Bob to see the link
    retry_fn_until_timeout(
        || async {
            let links: Vec<Link> = conductors[1]
                .call(&bob_zome, "get_links", base.clone())
                .await;

            links.iter().any(|link| link.create_link_hash == link_hash)
        },
        None,
        None,
    )
    .await
    .unwrap();

    // Bob deletes the link
    conductors[1]
        .call::<_, ()>(&bob_zome, "delete_link", link_hash.clone())
        .await;

    // Bob should no longer see the link
    assert!(conductors[1]
        .call::<_, Vec<Link>>(&bob_zome, "get_links", base.clone())
        .await
        .iter()
        .all(|link| { link.create_link_hash != link_hash }));
}
