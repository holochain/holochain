use holochain_cascade::test_utils::*;
use holochain_cascade::Cascade;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::mutations::insert_op_scratch;
use holochain_state::prelude::test_authored_db;
use holochain_state::prelude::test_cache_db;
use holochain_state::prelude::test_dht_db;
use holochain_state::scratch::Scratch;
use holochain_types::link::WireLinkOps;
use holochain_zome_types::ChainTopOrdering;

#[tokio::test(flavor = "multi_thread")]
async fn links_not_authority() {
    holochain_trace::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td = EntryTestData::create();
    fill_db(&authority.to_db(), td.store_entry_op.clone());
    fill_db(&authority.to_db(), td.create_link_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.to_db());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    let r = cascade
        .get_link_details(td.link_key.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, vec![(td.create_link_action.clone(), vec![]),]);

    fill_db(&authority.to_db(), td.delete_link_op.clone());

    let r = cascade
        .dht_get_links(td.link_key.clone(), Default::default())
        .await
        .unwrap();

    assert!(r.is_empty());

    let r = cascade
        .get_link_details(td.link_key.clone(), Default::default())
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
    fill_db(&vault.to_db(), td.store_entry_op.clone());
    fill_db(&vault.to_db(), td.create_link_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_network(mock, cache.to_db())
        .with_authored(vault.to_db().into());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    fill_db(&vault.to_db(), td.delete_link_op.clone());

    let r = cascade
        .dht_get_links(td.link_key.clone(), Default::default())
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
    let mut cascade = Cascade::empty()
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

    let mut cascade = Cascade::empty()
        .with_network(mock, cache.to_db())
        .with_scratch(scratch.into_sync());

    let r = cascade
        .dht_get_links(td.link_key.clone(), Default::default())
        .await
        .unwrap();

    assert!(r.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "todo"]
async fn test_links_can_match_a_partial_tag() {
    todo!()
}
