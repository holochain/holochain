use holo_hash::HasHash;
use holochain_cascade::test_utils::*;
use holochain_cascade::Cascade;
use holochain_p2p::HolochainP2pDnaT;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::mutations::insert_op_scratch;
use holochain_state::prelude::test_authored_db;
use holochain_state::prelude::test_cache_db;
use holochain_state::prelude::test_dht_db;
use holochain_state::scratch::Scratch;
use holochain_zome_types::ChainTopOrdering;
use holochain_zome_types::Details;
use holochain_zome_types::EntryDetails;
use holochain_zome_types::EntryDhtStatus;
use holochain_zome_types::GetOptions;
use holochain_zome_types::RecordDetails;
use holochain_zome_types::ValidationStatus;

async fn assert_can_get<N: HolochainP2pDnaT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    td_record: &RecordTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Get via entry hash
    let r = cascade
        .dht_get(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get entry");

    assert_eq!(*r.action_address(), td_entry.create_hash);
    assert_eq!(r.action().entry_hash(), Some(&td_entry.hash));

    // - Get via action hash
    let r = cascade
        .dht_get(td_record.any_action_hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get record");

    assert_eq!(*r.action_address(), td_record.any_action_hash);
    assert_eq!(r.action().entry_hash(), td_record.any_entry_hash.as_ref());

    // - Get details via entry hash
    let r = cascade
        .get_details(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get entry");

    let expected = Details::Entry(EntryDetails {
        entry: td_entry.entry.entry.clone(),
        actions: vec![td_entry
            .wire_create
            .data
            .clone()
            .into_action(td_entry.entry.entry_type.clone(), td_entry.hash.clone())],
        rejected_actions: vec![],
        deletes: vec![],
        updates: vec![],
        entry_dht_status: EntryDhtStatus::Live,
    });

    assert_eq!(r, expected);

    // - Get details via action hash
    let r = cascade
        .get_details(td_record.any_action_hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get record details");

    let expected = Details::Record(RecordDetails {
        record: td_record.any_record.clone(),
        validation_status: ValidationStatus::Valid,
        deletes: vec![],
        updates: vec![],
    });
    assert_eq!(r, expected);
}

async fn assert_is_none<N: HolochainP2pDnaT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    td_record: &RecordTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Get via entry hash
    let r = cascade
        .dht_get(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get via action hash
    let r = cascade
        .dht_get(td_record.any_action_hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get details via entry hash
    let r = cascade
        .get_details(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get details via action hash
    let r = cascade
        .get_details(td_record.any_action_hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());
}

async fn assert_rejected<N: HolochainP2pDnaT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    td_record: &RecordTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Get via entry hash
    let r = cascade
        .dht_get(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get via action hash
    let r = cascade
        .dht_get(td_record.any_action_hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    let r = cascade
        .get_details(td_entry.hash.clone().into(), Default::default())
        .await
        .unwrap()
        .expect("Failed to get entry");

    let expected = Details::Entry(EntryDetails {
        entry: td_entry.entry.entry.clone(),
        actions: vec![],
        rejected_actions: vec![td_entry
            .wire_create
            .data
            .clone()
            .into_action(td_entry.entry.entry_type.clone(), td_entry.hash.clone())],
        deletes: vec![],
        updates: vec![],
        entry_dht_status: EntryDhtStatus::Dead,
    });

    assert_eq!(r, expected);

    let r = cascade
        .get_details(td_record.any_action_hash.clone().into(), Default::default())
        .await
        .unwrap()
        .expect("Failed to get entry");

    let expected = Details::Record(RecordDetails {
        record: td_record.any_record.clone(),
        validation_status: ValidationStatus::Rejected,
        deletes: vec![],
        updates: vec![],
    });

    assert_eq!(r, expected);
}

async fn assert_can_retrieve<N: HolochainP2pDnaT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Retrieve via entry hash
    let r = cascade
        .retrieve(td_entry.hash.clone().into(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve record");

    assert_eq!(*r.action_address(), td_entry.create_hash);
    assert_eq!(r.action().entry_hash(), Some(&td_entry.hash));

    // - Retrieve via action hash
    let r = cascade
        .retrieve(td_entry.create_hash.clone().into(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve record");

    assert_eq!(*r.action_address(), td_entry.create_hash);
    assert_eq!(r.action().entry_hash(), Some(&td_entry.hash));

    // - Retrieve entry
    let r = cascade
        .retrieve_entry(td_entry.hash.clone(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve entry");

    assert_eq!(*r.as_hash(), td_entry.hash);

    // - Retrieve action
    let r = cascade
        .retrieve_action(td_entry.create_hash.clone(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve action");

    assert_eq!(*r.as_hash(), td_entry.create_hash);
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_not_authority_or_authoring() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    fill_db(&authority.to_db(), td_entry.store_entry_op.clone());
    fill_db(&authority.to_db(), td_record.any_store_record_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.to_db());

    assert_can_get(&td_entry, &td_record, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_authoring() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let mut scratch = Scratch::new();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    insert_op_scratch(
        &mut scratch,
        td_entry.store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op_scratch(
        &mut scratch,
        td_record.any_store_record_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_scratch(scratch.into_sync())
        .with_network(mock, cache.to_db());

    assert_can_get(&td_entry, &td_record, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_authority() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let vault = test_authored_db();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    fill_db(&vault.to_db(), td_entry.store_entry_op.clone());
    fill_db(&vault.to_db(), td_record.any_store_record_op.clone());

    // Network
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    mock.expect_get().returning(|_, _| Ok(vec![]));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_authored(vault.to_db().into())
        .with_network(mock, cache.to_db());

    assert_can_get(&td_entry, &td_record, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn content_not_authority_or_authoring() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let vault = test_authored_db();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    fill_db(&vault.to_db(), td_entry.store_entry_op.clone());
    fill_db(&vault.to_db(), td_record.any_store_record_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_authored(vault.to_db().into())
        .with_network(mock, cache.to_db());

    assert_can_get(&td_entry, &td_record, &mut cascade, GetOptions::content()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn content_authoring() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let mut scratch = Scratch::new();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    insert_op_scratch(
        &mut scratch,
        td_entry.store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op_scratch(
        &mut scratch,
        td_record.any_store_record_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_scratch(scratch.into_sync())
        .with_network(mock, cache.to_db());

    assert_can_get(&td_entry, &td_record, &mut cascade, GetOptions::content()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn content_authority() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let vault = test_authored_db();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_authored(vault.to_db().into())
        .with_network(mock, cache.to_db());

    assert_is_none(&td_entry, &td_record, &mut cascade, GetOptions::content()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_ops() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    fill_db_rejected(&authority.to_db(), td_entry.store_entry_op.clone());
    fill_db_rejected(&authority.to_db(), td_record.any_store_record_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.to_db());
    assert_rejected(&td_entry, &td_record, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn check_can_handle_rejected_ops_in_cache() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    fill_db_rejected(&cache.to_db(), td_entry.store_entry_op.clone());
    fill_db_rejected(&cache.to_db(), td_record.any_store_record_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.to_db());
    assert_rejected(&td_entry, &td_record, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "todo"]
async fn check_all_queries_still_work() {
    // TODO: Come up with a list of different states the authority could
    // have data in (updates, rejected, abandoned, nothing etc.)
    // then create an iterator that can put databases in these states and
    // run all the above queries on them.
    todo!()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "todo"]
async fn check_all_queries_still_work_with_cache() {
    todo!()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "todo"]
async fn check_all_queries_still_work_with_scratch() {
    todo!()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_pending_data_isnt_returned() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();
    let vault = test_authored_db();

    // Data
    let td_entry = EntryTestData::create();
    let td_record = RecordTestData::create();
    fill_db_pending(&authority.to_db(), td_entry.store_entry_op.clone());
    fill_db_pending(&authority.to_db(), td_record.any_store_record_op.clone());
    fill_db_pending(&vault.to_db(), td_entry.store_entry_op.clone());
    fill_db_pending(&vault.to_db(), td_record.any_store_record_op.clone());
    fill_db_pending(&cache.to_db(), td_entry.store_entry_op.clone());
    fill_db_pending(&cache.to_db(), td_record.any_store_record_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.to_db());

    assert_is_none(&td_entry, &td_record, &mut cascade, GetOptions::latest()).await;

    assert_can_retrieve(&td_entry, &mut cascade, GetOptions::latest()).await;

    let network = PassThroughNetwork::authority_for_all(vec![authority.to_db().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.to_db());

    assert_is_none(&td_entry, &td_record, &mut cascade, GetOptions::latest()).await;

    assert_can_retrieve(&td_entry, &mut cascade, GetOptions::latest()).await;
}
