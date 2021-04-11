use ghost_actor::dependencies::observability;
use holochain_cascade2::test_utils::*;
use holochain_cascade2::Cascade;
use holochain_state::insert::insert_op_scratch;
use holochain_state::prelude::test_cell_env;
use holochain_state::scratch::Scratch;
use holochain_zome_types::GetOptions;

#[tokio::test(flavor = "multi_thread")]
async fn entry_not_authority_or_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let authority = test_cell_env();

    // Data
    let td = EntryTestData::new();
    fill_db(&authority.env(), td.store_entry_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::<PassThroughNetwork>::empty().with_network(network, cache.env());

    let r = cascade
        .dht_get(td.hash.clone().into(), Default::default())
        .await
        .unwrap()
        .expect("Failed to get entry");

    assert_eq!(*r.header_address(), td.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td.hash));
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let mut scratch = Scratch::new();

    // Data
    let td = EntryTestData::new();
    insert_op_scratch(&mut scratch, td.store_entry_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT2::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::<MockNetwork>::empty()
        .with_scratch(scratch)
        .with_network(mock, cache.env());

    let r = cascade
        .dht_get(td.hash.clone().into(), Default::default())
        .await
        .unwrap()
        .expect("Failed to get entry");

    assert_eq!(*r.header_address(), td.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td.hash));
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_authority() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td = EntryTestData::new();
    fill_db(&vault.env(), td.store_entry_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT2::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::<MockNetwork>::empty()
        .with_vault(vault.env().into())
        .with_network(mock, cache.env());

    let r = cascade
        .dht_get(td.hash.clone().into(), Default::default())
        .await
        .unwrap()
        .expect("Failed to get entry");

    assert_eq!(*r.header_address(), td.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td.hash));
}

#[tokio::test(flavor = "multi_thread")]
async fn content_not_authority_or_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td = EntryTestData::new();
    fill_db(&vault.env(), td.store_entry_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT2::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::<MockNetwork>::empty()
        .with_vault(vault.env().into())
        .with_network(mock, cache.env());

    let r = cascade
        .dht_get(td.hash.clone().into(), GetOptions::content())
        .await
        .unwrap()
        .expect("Failed to get entry");

    assert_eq!(*r.header_address(), td.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td.hash));
}

#[tokio::test(flavor = "multi_thread")]
async fn content_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let mut scratch = Scratch::new();

    // Data
    let td = EntryTestData::new();
    insert_op_scratch(&mut scratch, td.store_entry_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT2::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::<MockNetwork>::empty()
        .with_scratch(scratch)
        .with_network(mock, cache.env());

    let r = cascade
        .dht_get(td.hash.clone().into(), GetOptions::content())
        .await
        .unwrap()
        .expect("Failed to get entry");

    assert_eq!(*r.header_address(), td.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td.hash));
}

#[tokio::test(flavor = "multi_thread")]
async fn content_authority() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td = EntryTestData::new();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT2::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::<MockNetwork>::empty()
        .with_vault(vault.env().into())
        .with_network(mock, cache.env());

    let r = cascade
        .dht_get(td.hash.clone().into(), Default::default())
        .await
        .unwrap();

    assert!(r.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_ops() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let authority = test_cell_env();

    // Data
    let td = EntryTestData::new();
    fill_db_rejected(&authority.env(), td.store_entry_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::<PassThroughNetwork>::empty().with_network(network, cache.env());

    let r = cascade
        .dht_get(td.hash.clone().into(), Default::default())
        .await
        .unwrap();

    assert!(r.is_none());
}
