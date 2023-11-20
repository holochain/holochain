use holochain_cascade::test_utils::*;
use holochain_cascade::CascadeImpl;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn links_not_authority() {
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

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    let r = cascade
        .get_link_details(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, vec![(td.create_link_action.clone(), vec![]),]);

    fill_db(&authority.to_db(), td.delete_link_op.clone()).await;

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert!(r.is_empty());

    let r = cascade
        .get_link_details(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(
        r,
        vec![(
            td.create_link_action.clone(),
            vec![td.delete_link_action.clone()]
        ),]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn links_authority() {
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
    let mock = MockNetwork::new(mock);

    // Cascade
    let cascade = CascadeImpl::empty()
        .with_network(mock, cache.to_db())
        .with_authored(vault.to_db().into());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    fill_db(&vault.to_db(), td.delete_link_op.clone()).await;

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert!(r.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn links_authoring() {
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
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    mock.expect_get_links().returning(|_, _| {
        Ok(vec![WireLinkOps {
            creates: vec![],
            deletes: vec![],
        }])
    });
    let mock = MockNetwork::new(mock);

    // Cascade
    let cascade = CascadeImpl::empty()
        .with_network(mock.clone(), cache.to_db())
        .with_scratch(scratch.clone().into_sync());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    insert_op_scratch(
        &mut scratch,
        td.delete_link_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();

    let cascade = CascadeImpl::empty()
        .with_network(mock, cache.to_db())
        .with_scratch(scratch.into_sync());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert!(r.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_links_can_match_a_partial_tag() {
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
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    mock.expect_get_links().returning(|_, _| {
        Ok(vec![WireLinkOps {
            creates: vec![],
            deletes: vec![],
        }])
    });
    let mock = MockNetwork::new(mock);

    // Cascade
    let cascade = CascadeImpl::empty()
        .with_network(mock.clone(), cache.to_db())
        .with_scratch(scratch.clone().into_sync());

    let mut query = td.link_key_tag.clone();
    // Take the first 10 bytes of the tag
    query.tag = Some(LinkTag::new(
        query
            .tag
            .unwrap()
            .0
            .into_iter()
            .take(10)
            .collect::<Vec<u8>>(),
    ));

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(1, r.len());
}
