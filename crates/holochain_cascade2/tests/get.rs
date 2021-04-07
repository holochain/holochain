use ghost_actor::dependencies::observability;
use holochain_cascade2::test_utils::*;
use holochain_cascade2::Cascade;
use holochain_state::prelude::test_cell_env;

#[tokio::test(flavor = "multi_thread")]
async fn entry_not_authority_or_author() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let other = test_cell_env();

    // Data
    let td = EntryTestData::new();
    fill_db(&other.env(), td.store_entry_op.clone());

    // Network
    let network = PassThroughNetwork(vec![other.env().clone().into()]);

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
async fn entry_author() {
    observability::test_run().ok();

    // Environments
    let vault = test_cell_env();
    let cache = test_cell_env();

    // Data
    let td = EntryTestData::new();
    fill_db_as_author(&vault.env(), td.store_entry_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mock = MockHolochainP2pCellT2::new();
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
async fn entry_authority() {
    observability::test_run().ok();
    todo!();
}

#[tokio::test(flavor = "multi_thread")]
async fn content_not_authority_or_author() {
    observability::test_run().ok();
    todo!();
}

#[tokio::test(flavor = "multi_thread")]
async fn content_author() {
    observability::test_run().ok();
    todo!();
}

#[tokio::test(flavor = "multi_thread")]
async fn content_authority() {
    observability::test_run().ok();
    todo!();
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_ops() {
    observability::test_run().ok();
    todo!();
}
