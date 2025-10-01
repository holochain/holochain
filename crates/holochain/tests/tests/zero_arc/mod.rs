use hdk::prelude::{
    Entry, EntryDef, EntryDefIndex, EntryHashed, EntryVisibility, MustGetActionInput,
    MustGetEntryInput, Record,
};
use holo_hash::{ActionHash, EntryHash};
use holochain::prelude::SignedActionHashed;
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
        .function("create", {
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
        .function("must_get_action", {
            move |api, hash: ActionHash| {
                let action = api.must_get_action(MustGetActionInput::new(hash))?;
                Ok(action)
            }
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
