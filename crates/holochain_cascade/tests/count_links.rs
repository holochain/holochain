use std::sync::Arc;

use holochain_cascade::test_utils::*;
use holochain_cascade::CascadeImpl;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::prelude::*;
use holochain_types::test_utils::chain::action_hash;

// Checks that links can be counted by asking a remote peer who is an authority on the base for the count
#[tokio::test(flavor = "multi_thread")]
async fn count_links_not_authority() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td = EntryTestData::create();
    fill_db(&authority.to_db(), td.store_entry_op.clone()).await;
    fill_db(&authority.to_db(), td.create_link_op.clone()).await;

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let cascade = CascadeImpl::empty().with_network(network, cache.to_db());

    let count = cascade
        .dht_count_links(td.link_query.clone())
        .await
        .unwrap();

    assert_eq!(count, td.links.len());

    fill_db(&authority.to_db(), td.delete_link_op.clone()).await;

    let count = cascade
        .dht_count_links(td.link_query.clone())
        .await
        .unwrap();

    assert_eq!(count, 0);
}

// Checks that network access is not required for an authority, the agent can count links locally
#[tokio::test(flavor = "multi_thread")]
async fn count_links_authority() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let vault = test_authored_db();

    // Data
    let td = EntryTestData::create();
    fill_db(&vault.to_db(), td.store_entry_op.clone()).await;
    fill_db(&vault.to_db(), td.create_link_op.clone()).await;

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    let mock = Arc::new(mock);

    // Cascade
    let cascade = CascadeImpl::empty()
        .with_network(mock, cache.to_db())
        .with_authored(vault.to_db().into());

    let count = cascade
        .dht_count_links(td.link_query.clone())
        .await
        .unwrap();

    assert_eq!(count, td.links.len());

    fill_db(&vault.to_db(), td.delete_link_op.clone()).await;

    let count = cascade
        .dht_count_links(td.link_query.clone())
        .await
        .unwrap();

    assert_eq!(count, 0);
}

// Checks that locally authored data that hasn't yet been published to the network is included in the link count
// seen by the agent doing the publish
#[tokio::test(flavor = "multi_thread")]
async fn count_links_authoring() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let mut scratch = Scratch::new();

    // Data
    let td = EntryTestData::create();
    insert_op_scratch(
        &mut scratch,
        td.store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op_scratch(
        &mut scratch,
        td.create_link_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();

    // Network
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    mock.expect_count_links()
        .returning(|_| Ok(CountLinksResponse::new(vec![action_hash(&[1, 2, 3])])));
    let mock = Arc::new(mock);

    // Cascade
    let cascade = CascadeImpl::empty()
        .with_network(mock.clone(), cache.to_db())
        .with_scratch(scratch.clone().into_sync());

    let count = cascade
        .dht_count_links(td.link_query.clone())
        .await
        .unwrap();

    // plus 1 to account for the remote link that we aren't storing
    assert_eq!(count, td.links.len() + 1);

    insert_op_scratch(
        &mut scratch,
        td.delete_link_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();

    let cascade = CascadeImpl::empty()
        .with_network(mock, cache.to_db())
        .with_scratch(scratch.into_sync());

    let count = cascade
        .dht_count_links(td.link_query.clone())
        .await
        .unwrap();

    // Our link has been deleted but the other link that the remote knows about remains
    assert_eq!(count, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn count_links_with_filters() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td = EntryTestData::create();
    fill_db(&authority.to_db(), td.store_entry_op.clone()).await;
    fill_db(&authority.to_db(), td.create_link_op.clone()).await;

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let cascade = CascadeImpl::empty().with_network(network, cache.to_db());

    // Negative check for `after`
    let mut query = td.link_query.clone();
    query.after = Some(Timestamp::now());
    assert_eq!(0, execute_query(&cascade, query).await);

    // Positive check for `after`
    let mut query = td.link_query.clone();
    query.after = Some(Timestamp::MIN);
    assert_eq!(td.links.len(), execute_query(&cascade, query).await);

    // Negative check for `before`
    let mut query = td.link_query.clone();
    query.before = Some(Timestamp::MIN);
    assert_eq!(0, execute_query(&cascade, query).await);

    // Positive check for `before`
    let mut query = td.link_query.clone();
    query.before = Some(Timestamp::now());
    assert_eq!(td.links.len(), execute_query(&cascade, query).await);

    // Negative check for `author`
    let mut query = td.link_query.clone();
    query.author = Some(fake_agent_pub_key(2));
    assert_eq!(0, execute_query(&cascade, query).await);

    // Positive check for `author`
    let mut query = td.link_query.clone();
    query.author = td.links.first().map(|l| l.author.clone());
    assert_eq!(td.links.len(), execute_query(&cascade, query).await);
}

async fn execute_query(cascade: &CascadeImpl, query: WireLinkQuery) -> usize {
    cascade.dht_count_links(query).await.unwrap()
}
