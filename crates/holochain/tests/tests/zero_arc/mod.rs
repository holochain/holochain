use hdk::link::GetLinksInputBuilder;
use hdk::prelude::{
    Entry, EntryDef, EntryDefIndex, EntryVisibility, LinkType, LinkTypeFilter, Op, Record,
    ValidateCallbackResult,
};
use holo_hash::ExternalHash;
use holochain::sweettest::{
    SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes,
};
use holochain_state::prelude::MustGetValidRecordInput;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::CreateInput;
use holochain_zome_types::prelude::CreateLinkInput;

/// Test that zero arc nodes can use must_get_valid_record to get dependencies during validation.
#[tokio::test(flavor = "multi_thread")]
async fn must_get_valid_record() {
    holochain_trace::test_run();

    let base_addr = ExternalHash::from_raw_32(vec![0; 32]);

    let entry_def = EntryDef::default_from_id("entry");
    let zomes = SweetInlineZomes::new(vec![entry_def], 1)
        .function("create", {
            let base_addr = base_addr.clone();
            move |api, _: ()| {
                let entry = Entry::app(().try_into().unwrap()).unwrap();
                let hash = api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                    EntryVisibility::Public,
                    entry,
                    ChainTopOrdering::default(),
                ))?;
                api.create_link(CreateLinkInput::new(
                    base_addr.clone().into(),
                    hash.into(),
                    0.into(),
                    LinkType(0),
                    ().into(),
                    ChainTopOrdering::default(),
                ))?;

                Ok(())
            }
        })
        .function("get", move |api, _: ()| {
            let links = api.get_links(vec![GetLinksInputBuilder::try_new(
                base_addr.clone(),
                LinkTypeFilter::Types(vec![(0.into(), vec![LinkType(0)])]),
            )
            .unwrap()
            .build()])?;

            let mut out = vec![];
            for link in links.into_iter().flatten() {
                let record = api.must_get_valid_record(MustGetValidRecordInput::new(
                    link.target.try_into().unwrap(),
                ))?;
                out.push(record);
            }

            Ok(out)
        })
        .integrity_function("validate", |_, _: Op| Ok(ValidateCallbackResult::Valid))
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

    // Alice creates an entry
    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    let _: () = conductors[0].call(&alice_zome, "create", ()).await;

    // Ensure Bob cannot see Alice's entry
    let bob_zome = bob.zome(SweetInlineZomes::COORDINATOR);
    let records: Vec<Record> = conductors[1].call(&bob_zome, "get", ()).await;

    assert!(
        records.is_empty(),
        "Expected Bob to not be able to get Alice's record"
    );

    // Simulate Alice reaching a full storage arc
    conductors[0]
        .declare_full_storage_arcs(test_dna.dna_hash())
        .await;
    // and ensure Bob knows about Alice's full arc.
    conductors.exchange_peer_info().await;

    // Now Bob should be able to get Alice's entry
    let records: Vec<Record> = conductors[1].call(&bob_zome, "get", ()).await;
    assert_eq!(
        records.len(),
        1,
        "Expected Bob to be able to get Alice's record"
    );
}
