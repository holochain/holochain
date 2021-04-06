use ghost_actor::dependencies::observability;
use holochain_cascade2::test_utils::*;
use holochain_cascade2::Cascade;
use holochain_state::prelude::test_cell_env;

#[tokio::test(flavor = "multi_thread")]
async fn entry_not_authority_or_author() {
    observability::test_run().ok();

    // Environments
    let other = test_cell_env();

    // Data
    let td = EntryTestData::new();
    fill_db(&other.env(), td.store_entry_op.clone());

    let network = PassThroughNetwork(vec![other.env().clone().into()]);
    let mut cascade = Cascade::<PassThroughNetwork>::empty().with_network(network);

    cascade
        .dht_get(td.hash.clone().into(), Default::default())
        .await
        .unwrap()
        .expect("Failed to get entry");
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_author() {
    observability::test_run().ok();
    todo!();
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
