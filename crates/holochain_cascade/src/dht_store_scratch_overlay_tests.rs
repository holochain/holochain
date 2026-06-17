use super::*;
use ::fixt::fixt;
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain_zome_types::action::{Action, ActionHashed, ChainTopOrdering};

async fn empty_store() -> holochain_state::dht_store::DhtStore {
    let dna_hash = holo_hash::DnaHash::from_raw_36(vec![42u8; 36]);
    holochain_state::test_utils::test_dht_store(dna_hash).await
}

/// A `dht_get_action` call on a `CascadeImpl` built with a `DhtStore` plus a
/// scratch that holds an authored action must return that action without
/// reaching the network (the cascade has no network handle).
#[tokio::test]
async fn dht_get_action_reflects_scratch_action() {
    let store = empty_store().await;

    // Build a signed action with no associated entry (Dna action type).
    let action = Action::Dna(fixt!(Dna));
    let sah = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(action),
        fixt!(Signature),
    );
    let action_hash = sah.as_hash().clone();

    // Put the action into the scratch.
    let mut scratch = Scratch::new();
    scratch.add_action(sah.clone(), ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    // Construct a cascade with the DhtStore + scratch, but NO network.
    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let result = cascade
        .dht_get_action(action_hash.clone(), GetOptions::local())
        .await
        .expect("dht_get_action");

    let record = result.expect("expected Some(Record) from scratch");
    assert_eq!(
        record.action_address(),
        &action_hash,
        "returned action hash must match the scratched action"
    );
}

/// A `dht_get_entry` call on a `CascadeImpl` with a scratch holding a
/// `Create` action + entry returns that record without a network request.
#[tokio::test]
async fn dht_get_entry_reflects_scratch_create() {
    use holochain_zome_types::action::{Create, EntryType};
    use holochain_zome_types::entry::Entry;
    use holochain_zome_types::prelude::EntryHashed;

    let store = empty_store().await;

    // Build an agent entry (simplest public entry type).
    let agent = fixt!(AgentPubKey);
    let entry = Entry::Agent(agent.clone());
    let entry_hash = holo_hash::EntryHash::with_data_sync(&entry);

    let create_action = Action::Create(Create {
        author: agent.clone(),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(0),
        action_seq: 1,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![0u8; 36]),
        entry_type: EntryType::AgentPubKey,
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let sah = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(create_action),
        fixt!(Signature),
    );

    let mut scratch = Scratch::new();
    scratch.add_action(sah.clone(), ChainTopOrdering::Relaxed);
    let entry_hashed = EntryHashed::from_content_sync(entry);
    scratch.add_entry(entry_hashed, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let result = cascade
        .dht_get_entry(entry_hash.clone(), GetOptions::local())
        .await
        .expect("dht_get_entry");

    let record = result.expect("expected Some(Record) for scratch entry");
    assert_eq!(
        record.action().entry_hash(),
        Some(&entry_hash),
        "returned record entry hash must match the scratched entry"
    );
}

/// Helper: integrate a `StoreRecord` op for a `Create` action so that
/// `get_record_details_with_scratch` can locate the action via its store gate.
///
/// The entry is included in the op so `retrieve_record` can locate it in
/// the database when assembling `RecordDetails`.
async fn integrate_store_record(
    store: &holochain_state::dht_store::DhtStore,
    seed: u8,
    author: &AgentPubKey,
    entry_hash: holo_hash::EntryHash,
    entry: holochain_zome_types::entry::Entry,
) -> ActionHash {
    use holochain_state::dht_store::{AppOutcome, SysOutcome};
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_types::prelude::RecordEntry;
    use holochain_zome_types::action::{AppEntryDef, Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::Signature;

    let action = Action::Create(Create {
        author: author.clone(),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let action_hash = holo_hash::ActionHash::with_data_sync(&action);
    let chain_op = ChainOp::StoreRecord(
        Signature::from([seed; 64]),
        action,
        RecordEntry::Present(entry),
    );
    let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op)));
    let op_hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op_hash.clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(holochain_zome_types::prelude::Timestamp::from_micros(1000))
        .await
        .unwrap();
    action_hash
}

/// Helper: integrate a `StoreEntry` op for a `Create` action.
async fn integrate_store_entry(
    store: &holochain_state::dht_store::DhtStore,
    seed: u8,
    author: &AgentPubKey,
    entry_hash: holo_hash::EntryHash,
    entry: holochain_zome_types::entry::Entry,
) -> ActionHash {
    use holochain_state::dht_store::{AppOutcome, SysOutcome};
    use holochain_types::action::NewEntryAction;
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_zome_types::action::{AppEntryDef, Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::Signature;

    let action = Action::Create(Create {
        author: author.clone(),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let action_hash = holo_hash::ActionHash::with_data_sync(&action);
    let new_entry_action = match action {
        Action::Create(c) => NewEntryAction::Create(c),
        _ => unreachable!(),
    };
    let chain_op = ChainOp::StoreEntry(Signature::from([seed; 64]), new_entry_action, entry);
    let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op)));
    let op_hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op_hash.clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(holochain_zome_types::prelude::Timestamp::from_micros(1000))
        .await
        .unwrap();
    action_hash
}

/// `get_record_details` on a `CascadeImpl` with a `DhtStore` plus a scratch
/// that holds a `Delete` targeting an integrated record must return
/// `RecordDetails` that includes the scratch delete.
#[tokio::test]
async fn get_record_details_reflects_scratch_delete() {
    use holochain_zome_types::action::Delete;
    use holochain_zome_types::entry::Entry;

    let store = empty_store().await;

    let seed = 42u8;
    let author = AgentPubKey::from_raw_36(vec![seed; 36]);

    // Build an Agent entry so retrieve_record can fetch it from the DB.
    let agent_key = AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]);
    let entry = Entry::Agent(agent_key.clone());
    let entry_hash = holo_hash::EntryHash::with_data_sync(&entry);

    // Integrate a StoreRecord op so get_record_details_with_scratch can find it.
    let action_hash =
        integrate_store_record(&store, seed, &author, entry_hash.clone(), entry).await;

    // Put a scratch Delete targeting the integrated action into the scratch.
    let delete = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(seed as i64 * 3000),
        action_seq: 3,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(160); 36]),
        deletes_address: action_hash.clone(),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_sah = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(delete),
        fixt!(Signature),
    );

    let mut scratch = Scratch::new();
    scratch.add_action(delete_sah, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let options = CascadeOptions {
        get_options: GetOptions::local(),
        network_request_options: GetOptions::local().to_network_options(),
    };

    let details = cascade
        .get_record_details(action_hash.clone(), options)
        .await
        .expect("get_record_details")
        .expect("expected Some(RecordDetails) for integrated record");

    assert_eq!(
        details.deletes.len(),
        1,
        "scratch Delete must appear in RecordDetails.deletes"
    );
    assert_eq!(details.updates.len(), 0, "no updates expected");
}

/// `get_entry_details` on a `CascadeImpl` with a `DhtStore` plus a scratch
/// holding a `Delete` targeting the entry must return `EntryDetails` where
/// the scratch delete appears and `entry_dht_status` is `Dead`.
#[tokio::test]
async fn get_entry_details_reflects_scratch_delete() {
    use holochain_zome_types::action::Delete;
    use holochain_zome_types::entry::Entry;
    use holochain_zome_types::metadata::EntryDhtStatus;

    let store = empty_store().await;

    let seed = 43u8;
    let author = AgentPubKey::from_raw_36(vec![seed; 36]);

    // Use an Agent entry as the simplest public entry type.
    let agent_key = AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]);
    let entry = Entry::Agent(agent_key.clone());
    let entry_hash = holo_hash::EntryHash::with_data_sync(&entry);

    // Integrate a StoreEntry op so the entry exists in the store and is Live.
    let store_action_hash =
        integrate_store_entry(&store, seed, &author, entry_hash.clone(), entry).await;

    // Put a scratch Delete targeting that action into the scratch so the
    // entry becomes Dead.
    let delete = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(seed as i64 * 3000),
        action_seq: 3,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(160); 36]),
        deletes_address: store_action_hash.clone(),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_sah = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(delete),
        fixt!(Signature),
    );

    let mut scratch = Scratch::new();
    scratch.add_action(delete_sah, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let options = CascadeOptions {
        get_options: GetOptions::local(),
        network_request_options: GetOptions::local().to_network_options(),
    };

    let details = cascade
        .get_entry_details(entry_hash.clone(), options)
        .await
        .expect("get_entry_details")
        .expect("expected Some(EntryDetails)");

    assert_eq!(
        details.deletes.len(),
        1,
        "scratch Delete must appear in EntryDetails.deletes"
    );
    assert_eq!(
        details.entry_dht_status,
        EntryDhtStatus::Dead,
        "entry must be Dead after scratch Delete"
    );
}

/// Helper: integrate a `RegisterAddLink` op into the store, returning the action hash.
async fn integrate_link_op(
    store: &holochain_state::dht_store::DhtStore,
    base: &holo_hash::AnyLinkableHash,
    zome_index: u8,
    link_type: u8,
    tag_bytes: Vec<u8>,
    seed: u8,
) -> holo_hash::ActionHash {
    use holochain_state::dht_store::{AppOutcome, SysOutcome};
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_zome_types::action::CreateLink;
    use holochain_zome_types::link::LinkTag;

    let action = Action::CreateLink(CreateLink {
        author: AgentPubKey::from_raw_36(vec![seed; 36]),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(60); 36]),
        base_address: base.clone(),
        target_address: holo_hash::AnyLinkableHash::from_raw_36_and_type(
            vec![seed.wrapping_add(20); 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        ),
        zome_index: zome_index.into(),
        link_type: link_type.into(),
        tag: LinkTag(tag_bytes),
        weight: Default::default(),
    });
    let create_link = match action {
        Action::CreateLink(ref cl) => cl.clone(),
        _ => unreachable!(),
    };
    let action_hash = holo_hash::ActionHash::with_data_sync(&action);
    let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterAddLink(
        Signature::from([seed; 64]),
        create_link,
    ))));
    let op_hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op_hash, AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(holochain_zome_types::prelude::Timestamp::from_micros(1000))
        .await
        .unwrap();
    action_hash
}

/// Helper: build a scratch `CreateLink` action for `base`.
fn make_scratch_create_link(
    base: &holo_hash::AnyLinkableHash,
    zome_index: u8,
    link_type: u8,
    tag_bytes: Vec<u8>,
    seed: u8,
) -> SignedActionHashed {
    use holochain_zome_types::action::CreateLink;
    use holochain_zome_types::link::LinkTag;

    let action = Action::CreateLink(CreateLink {
        author: AgentPubKey::from_raw_36(vec![seed; 36]),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(60); 36]),
        base_address: base.clone(),
        target_address: holo_hash::AnyLinkableHash::from_raw_36_and_type(
            vec![seed.wrapping_add(20); 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        ),
        zome_index: zome_index.into(),
        link_type: link_type.into(),
        tag: LinkTag(tag_bytes),
        weight: Default::default(),
    });
    SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(action),
        Signature::from([seed; 64]),
    )
}

/// Helper: build a scratch `DeleteLink` action tombstoning `create_link_hash`.
fn make_scratch_delete_link(
    base: &holo_hash::AnyLinkableHash,
    create_link_hash: holo_hash::ActionHash,
    seed: u8,
) -> SignedActionHashed {
    use holochain_zome_types::action::DeleteLink;

    let action = Action::DeleteLink(DeleteLink {
        author: AgentPubKey::from_raw_36(vec![seed; 36]),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros(seed as i64 * 1000 + 500),
        action_seq: 3,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(90); 36]),
        base_address: base.clone(),
        link_add_address: create_link_hash,
    });
    SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(action),
        Signature::from([seed; 64]),
    )
}

/// `dht_get_links` via a `CascadeImpl` with a `DhtStore`:
///
/// - A scratch `CreateLink` appears in the result.
/// - A scratch `DeleteLink` tombstoning an integrated store link removes it.
#[tokio::test]
async fn dht_get_links_reflects_scratch_create_and_delete() {
    use holochain_p2p::actor::GetLinksRequestOptions;
    use holochain_zome_types::prelude::LinkTypeFilter;

    let store = empty_store().await;

    let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
        vec![50u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );

    // Integrate a store CreateLink for base (seed 51).
    let store_create_hash = integrate_link_op(&store, &base, 0, 0, vec![1, 2, 3], 51).await;

    // Add a scratch CreateLink for the same base (seed 52).
    let scratch_create_sah = make_scratch_create_link(&base, 0, 0, vec![1, 2, 3], 52);
    let scratch_create_hash = scratch_create_sah.as_hash().clone();

    // Add a scratch DeleteLink tombstoning the store CreateLink (seed 53).
    let scratch_delete_sah = make_scratch_delete_link(&base, store_create_hash.clone(), 53);

    let mut scratch = Scratch::new();
    scratch.add_action(scratch_create_sah, ChainTopOrdering::Relaxed);
    scratch.add_action(scratch_delete_sah, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let key = holochain_types::link::WireLinkKey {
        base: base.clone(),
        type_query: LinkTypeFilter::Dependencies(vec![0.into()]),
        tag: None,
        after: None,
        before: None,
        author: None,
    };
    let options = GetLinksRequestOptions {
        get_options: GetOptions::local(),
        ..Default::default()
    };

    let links = cascade
        .dht_get_links(key, options)
        .await
        .expect("dht_get_links");

    assert_eq!(links.len(), 1, "only the scratch CreateLink must be live");
    assert_eq!(
        links[0].create_link_hash, scratch_create_hash,
        "surviving link must be the scratch CreateLink"
    );
}

/// `get_links_details` via a `CascadeImpl` with a `DhtStore`:
///
/// - Shows a scratch `CreateLink`.
/// - Shows a scratch `DeleteLink` paired with its store `CreateLink`.
#[tokio::test]
async fn get_links_details_reflects_scratch_create_and_delete() {
    use holochain_p2p::actor::GetLinksRequestOptions;
    use holochain_zome_types::prelude::LinkTypeFilter;

    let store = empty_store().await;

    let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
        vec![60u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );

    // Integrate a store CreateLink (seed 61) — this will be tombstoned.
    let store_create_hash = integrate_link_op(&store, &base, 0, 0, vec![4, 5, 6], 61).await;

    // Add a scratch CreateLink (seed 62) — this will be live.
    let scratch_create_sah = make_scratch_create_link(&base, 0, 0, vec![4, 5, 6], 62);
    let scratch_create_hash = scratch_create_sah.as_hash().clone();

    // Add a scratch DeleteLink (seed 63) tombstoning the store create.
    let scratch_delete_sah = make_scratch_delete_link(&base, store_create_hash.clone(), 63);
    let scratch_delete_hash = scratch_delete_sah.as_hash().clone();

    let mut scratch = Scratch::new();
    scratch.add_action(scratch_create_sah, ChainTopOrdering::Relaxed);
    scratch.add_action(scratch_delete_sah, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let key = holochain_types::link::WireLinkKey {
        base: base.clone(),
        type_query: LinkTypeFilter::Dependencies(vec![0.into()]),
        tag: None,
        after: None,
        before: None,
        author: None,
    };
    let options = GetLinksRequestOptions {
        get_options: GetOptions::local(),
        ..Default::default()
    };

    let details = cascade
        .get_links_details(key, options)
        .await
        .expect("get_links_details");

    // Expect two creates: store create (tombstoned) + scratch create.
    assert_eq!(details.len(), 2, "both creates must appear in details");

    // Find the store create entry in details.
    let store_entry = details
        .iter()
        .find(|(sah, _)| sah.as_hash() == &store_create_hash)
        .expect("store CreateLink must appear in details");
    assert_eq!(
        store_entry.1.len(),
        1,
        "store CreateLink must have the scratch DeleteLink as its tombstone"
    );
    assert_eq!(
        store_entry.1[0].as_hash(),
        &scratch_delete_hash,
        "tombstone must be the scratch DeleteLink"
    );

    // Find the scratch create entry in details.
    let scratch_entry = details
        .iter()
        .find(|(sah, _)| sah.as_hash() == &scratch_create_hash)
        .expect("scratch CreateLink must appear in details");
    assert_eq!(
        scratch_entry.1.len(),
        0,
        "scratch CreateLink must have no deletes"
    );
}

/// `dht_count_links` via a `CascadeImpl` with a `DhtStore` (no network):
/// the scratch `CreateLink` is counted, and a scratch `DeleteLink`
/// tombstones the store `CreateLink`, so the unique count is 1.
#[tokio::test]
async fn dht_count_links_reflects_scratch_create_and_delete() {
    use holochain_zome_types::prelude::LinkTypeFilter;

    let store = empty_store().await;
    let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
        vec![60u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );

    // Store CreateLink (seed 61), a scratch CreateLink (62), and a scratch
    // DeleteLink (63) tombstoning the store create.
    let store_create_hash = integrate_link_op(&store, &base, 0, 0, vec![1, 2, 3], 61).await;
    let scratch_create_sah = make_scratch_create_link(&base, 0, 0, vec![1, 2, 3], 62);
    let scratch_delete_sah = make_scratch_delete_link(&base, store_create_hash, 63);

    let mut scratch = Scratch::new();
    scratch.add_action(scratch_create_sah, ChainTopOrdering::Relaxed);
    scratch.add_action(scratch_delete_sah, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let query = holochain_types::link::WireLinkQuery {
        base: base.clone(),
        link_type: LinkTypeFilter::Dependencies(vec![0.into()]),
        tag_prefix: None,
        before: None,
        after: None,
        author: None,
    };

    let count = cascade
        .dht_count_links(query)
        .await
        .expect("dht_count_links");
    assert_eq!(
        count, 1,
        "store create is tombstoned by the scratch delete; only the scratch create counts"
    );
}

// ---- agent-activity helpers ----

/// Integrate a `RegisterAgentActivity` op for the given `action` into the
/// store so it is marked accepted and integrated.
async fn integrate_activity_op(
    store: &holochain_state::dht_store::DhtStore,
    action: Action,
    seed: u8,
    when: i64,
) -> holo_hash::ActionHash {
    use holochain_state::dht_store::{AppOutcome, SysOutcome};
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};

    let action_hash = holo_hash::ActionHash::with_data_sync(&action);
    let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
        ChainOp::RegisterAgentActivity(Signature::from([seed; 64]), action),
    )));
    let op_hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op_hash, AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(holochain_zome_types::prelude::Timestamp::from_micros(when))
        .await
        .unwrap();
    action_hash
}

/// Build a `Create` action (no entry in store) for use as agent activity.
fn make_activity_create(
    author: &AgentPubKey,
    seq: u32,
    prev: &holo_hash::ActionHash,
    seed: u8,
) -> Action {
    use holochain_zome_types::action::{AppEntryDef, Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;

    Action::Create(Create {
        author: author.clone(),
        timestamp: holochain_zome_types::prelude::Timestamp::from_micros((seq as i64 + 1) * 1000),
        action_seq: seq,
        prev_action: prev.clone(),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: holo_hash::EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]),
        weight: Default::default(),
    })
}

/// Wrap an action as a `SignedActionHashed` for the scratch.
fn make_scratch_activity(action: Action, seed: u8) -> SignedActionHashed {
    SignedActionHashed::with_presigned(
        holochain_zome_types::action::ActionHashed::from_content_sync(action),
        Signature::from([seed; 64]),
    )
}

// ---- must_get_agent_activity tests ----

/// `must_get_agent_activity` via the cascade respects a scratch chain-top:
/// the chain top is in the scratch (not the store), so the response should
/// not be `ChainTopNotFound`.
#[tokio::test]
async fn must_get_agent_activity_reflects_scratch_chain_top() {
    let store = empty_store().await;
    let author = AgentPubKey::from_raw_36(vec![200u8; 36]);

    // Seq 0 in the store.
    let prev0 = holo_hash::ActionHash::from_raw_36(vec![0u8; 36]);
    let action0 = make_activity_create(&author, 0, &prev0, 200);
    let hash0 = integrate_activity_op(&store, action0, 200, 10).await;

    // Seq 1 (scratch-only): linked from hash0.
    let action1 = make_activity_create(&author, 1, &hash0, 201);
    let scratch_top_hash = holo_hash::ActionHash::with_data_sync(&action1);
    let sah1 = make_scratch_activity(action1, 201);

    let mut scratch = Scratch::new();
    scratch.add_action(sah1, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let filter = ChainFilter::new(scratch_top_hash.clone());
    let resp = cascade
        .must_get_agent_activity(author, filter, NetworkRequestOptions::default())
        .await
        .expect("must_get_agent_activity");

    assert!(
        !matches!(resp, MustGetAgentActivityResponse::ChainTopNotFound(_)),
        "scratch chain-top should have been resolved; got {resp:?}"
    );
}

/// `must_get_agent_activity` via the cascade includes scratch activity in
/// the merged result when the full chain is present (store + scratch).
#[tokio::test]
async fn must_get_agent_activity_reflects_scratch_activity() {
    let store = empty_store().await;
    let author = AgentPubKey::from_raw_36(vec![202u8; 36]);

    // Seqs 0..=1 in the store.
    let prev0 = holo_hash::ActionHash::from_raw_36(vec![0u8; 36]);
    let action0 = make_activity_create(&author, 0, &prev0, 202);
    let hash0 = integrate_activity_op(&store, action0, 202, 10).await;
    let action1 = make_activity_create(&author, 1, &hash0, 203);
    let hash1 = integrate_activity_op(&store, action1, 203, 11).await;

    // Seq 2 scratch-only, linked from hash1.
    let action2 = make_activity_create(&author, 2, &hash1, 204);
    let scratch_top_hash = holo_hash::ActionHash::with_data_sync(&action2);
    let sah2 = make_scratch_activity(action2, 204);

    let mut scratch = Scratch::new();
    scratch.add_action(sah2, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let filter = ChainFilter::new(scratch_top_hash.clone());
    let resp = cascade
        .must_get_agent_activity(author, filter, NetworkRequestOptions::default())
        .await
        .expect("must_get_agent_activity");

    match resp {
        MustGetAgentActivityResponse::Activity { activity, .. } => {
            let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
            assert!(
                seqs.contains(&2),
                "scratch action at seq 2 should be present; got {seqs:?}"
            );
            assert!(
                seqs.contains(&0),
                "store action at seq 0 should be present; got {seqs:?}"
            );
        }
        other => panic!("expected Activity, got {other:?}"),
    }
}

// ---- get_agent_activity tests ----

/// `get_agent_activity` via the cascade (requester, Local strategy) returns
/// scratch activity merged with the store.
#[tokio::test]
async fn get_agent_activity_reflects_scratch_activity() {
    let store = empty_store().await;
    let author = AgentPubKey::from_raw_36(vec![210u8; 36]);

    // Seq 0 in the store.
    let prev0 = holo_hash::ActionHash::from_raw_36(vec![0u8; 36]);
    let action0 = make_activity_create(&author, 0, &prev0, 210);
    integrate_activity_op(&store, action0, 210, 10).await;

    // Seq 1 scratch-only.
    let action1 = make_activity_create(
        &author,
        1,
        &holo_hash::ActionHash::from_raw_36(vec![211u8; 36]),
        211,
    );
    let sah1 = make_scratch_activity(action1, 211);

    let mut scratch = Scratch::new();
    scratch.add_action(sah1, ChainTopOrdering::Relaxed);
    let sync_scratch = scratch.into_sync();

    let cascade = CascadeImpl::empty(store).with_scratch(sync_scratch);

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_warrants: false,
        include_full_records: false,
        get_options: GetOptions::local(),
        ..Default::default()
    };
    let resp = cascade
        .get_agent_activity(author, ChainQueryFilter::new(), options)
        .await
        .expect("get_agent_activity");

    match &resp.valid_activity {
        ChainItems::Hashes(h) => {
            let seqs: Vec<u32> = h.iter().map(|(seq, _)| *seq).collect();
            assert!(
                seqs.contains(&0),
                "store action at seq 0 should be in valid activity; got {seqs:?}"
            );
            assert!(
                seqs.contains(&1),
                "scratch action at seq 1 should be in valid activity; got {seqs:?}"
            );
        }
        other => panic!("expected ChainItems::Hashes, got {other:?}"),
    }
}
