use fixt::prelude::*;
use ghost_actor::dependencies::observability;
use holochain_cascade::test_utils::*;
use holochain_cascade::Cascade;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::mutations::insert_op_scratch;
use holochain_state::prelude::test_cell_env;
use holochain_state::scratch::Scratch;
use holochain_types::link::WireLinkOps;
use holochain_zome_types::ChainTopOrdering;
use holochain_zome_types::ZomeFixturator;

#[tokio::test(flavor = "multi_thread")]
async fn links_not_authority() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let authority = test_cell_env();

    // Data
    let td = EntryTestData::create();
    fill_db(&authority.env(), td.store_entry_op.clone());
    fill_db(&authority.env(), td.create_link_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.env());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    let r = cascade
        .get_link_details(td.link_key.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, vec![(td.create_link_header.clone(), vec![]),]);

    fill_db(&authority.env(), td.delete_link_op.clone());

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
            td.create_link_header.clone(),
            vec![td.delete_link_header.clone()]
        ),]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn links_authority() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td = EntryTestData::create();
    fill_db(&vault.env(), td.store_entry_op.clone());
    fill_db(&vault.env(), td.create_link_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_, _| Ok(true));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_network(mock, cache.env())
        .with_vault(vault.env().into());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    fill_db(&vault.env(), td.delete_link_op.clone());

    let r = cascade
        .dht_get_links(td.link_key.clone(), Default::default())
        .await
        .unwrap();

    assert!(r.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn links_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let mut scratch = Scratch::new();
    let zome = fixt!(Zome);

    // Data
    let td = EntryTestData::create();
    insert_op_scratch(
        &mut scratch,
        Some(zome.clone()),
        td.store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op_scratch(
        &mut scratch,
        Some(zome.clone()),
        td.create_link_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pDnaT::new();
    mock.expect_authority_for_hash().returning(|_, _| Ok(false));
    mock.expect_get_links().returning(|_, _| {
        Ok(vec![WireLinkOps {
            creates: vec![],
            deletes: vec![],
        }])
    });
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_network(mock.clone(), cache.env())
        .with_scratch(scratch.clone().into_sync());

    let r = cascade
        .dht_get_links(td.link_key_tag.clone(), Default::default())
        .await
        .unwrap();

    assert_eq!(r, td.links);

    insert_op_scratch(
        &mut scratch,
        Some(zome),
        td.delete_link_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();

    let mut cascade = Cascade::empty()
        .with_network(mock, cache.env())
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
