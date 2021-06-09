use super::*;
use crate::authority::handle_get_agent_activity;
use crate::test_utils::*;
use ghost_actor::dependencies::observability;
use holochain_p2p::actor;
use holochain_p2p::event::GetRequest;
use holochain_state::prelude::test_cell_env;
use holochain_types::activity::ChainItems;

fn options() -> holochain_p2p::event::GetOptions {
    holochain_p2p::event::GetOptions {
        follow_redirects: false,
        // TODO: These are probably irrelevant now
        all_live_headers_with_metadata: true,
        request_type: Default::default(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = EntryTestData::create();

    fill_db(&env.env(), td.store_entry_op.clone());
    let options = options();

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.delete_entry_header_op.clone());

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![td.wire_delete.clone()],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.update_content_op.clone());

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![td.wire_delete.clone()],
        updates: vec![td.wire_update.clone()],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_element() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = ElementTestData::create();

    fill_db(&env.env(), td.store_element_op.clone());

    let options = options();

    let result = handle_get_element(env.env().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireElementOps {
        header: Some(td.wire_create.clone()),
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.deleted_by_op.clone());

    let result = handle_get_element(env.env().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireElementOps {
        header: Some(td.wire_create.clone()),
        deletes: vec![td.wire_delete.clone()],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.update_element_op.clone());

    let result = handle_get_element(env.env().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireElementOps {
        header: Some(td.wire_create.clone()),
        deletes: vec![td.wire_delete.clone()],
        updates: vec![td.wire_update.clone()],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.any_store_element_op.clone());

    let result = handle_get_element(
        env.env().into(),
        td.any_header_hash.clone(),
        options.clone(),
    )
    .await
    .unwrap();
    let expected = WireElementOps {
        header: Some(td.any_header.clone()),
        deletes: vec![],
        updates: vec![],
        entry: td.any_entry.clone(),
    };
    assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn retrieve_element() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = ElementTestData::create();

    fill_db_pending(&env.env(), td.store_element_op.clone());

    let mut options = options();
    options.request_type = GetRequest::Pending;

    let result = handle_get_element(env.env().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireElementOps {
        header: Some(td.wire_create.clone()),
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_links() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = EntryTestData::create();

    fill_db(&env.env(), td.store_entry_op.clone());
    fill_db(&env.env(), td.create_link_op.clone());
    let options = actor::GetLinksOptions::default();

    let result = handle_get_links(env.env().into(), td.link_key.clone(), (&options).into())
        .await
        .unwrap();
    let expected = WireLinkOps {
        creates: vec![td.wire_create_link.clone()],
        deletes: vec![],
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.delete_link_op.clone());

    let result = handle_get_links(env.env().into(), td.link_key_tag.clone(), (&options).into())
        .await
        .unwrap();
    let expected = WireLinkOps {
        creates: vec![td.wire_create_link_base.clone()],
        deletes: vec![td.wire_delete_link.clone()],
    };
    assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_agent_activity() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = ActivityTestData::valid_chain_scenario();

    for hash_op in td.hash_ops.iter().cloned() {
        fill_db(&env.env(), hash_op);
    }
    for hash_op in td.noise_ops.iter().cloned() {
        fill_db(&env.env(), hash_op);
    }

    let options = actor::GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_headers: false,
        ..Default::default()
    };

    let result = handle_get_agent_activity(
        env.env().into(),
        td.agent.clone(),
        td.query_filter.clone(),
        (&options).into(),
    )
    .await
    .unwrap();
    let mut expected = AgentActivityResponse {
        agent: td.agent.clone(),
        valid_activity: td.valid_hashes.clone(),
        rejected_activity: ChainItems::NotRequested,
        status: ChainStatus::Valid(td.chain_head.clone()),
        highest_observed: Some(td.highest_observed.clone()),
    };
    assert_eq!(result, expected);

    expected.valid_activity = match expected.valid_activity.clone() {
        ChainItems::Hashes(v) => ChainItems::Hashes(v.into_iter().take(20).collect()),
        _ => unreachable!(),
    };

    let filter = td.query_filter.sequence_range(0..20u32);
    let result = handle_get_agent_activity(
        env.env().into(),
        td.agent.clone(),
        filter,
        (&options).into(),
    )
    .await
    .unwrap();

    assert_eq!(result, expected);
}
