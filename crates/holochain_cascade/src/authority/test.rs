use super::*;
use crate::authority::handle_get_agent_activity;
use crate::test_utils::*;
use ghost_actor::dependencies::observability;
use holochain_p2p::actor;
use holochain_p2p::event::GetRequest;
use holochain_state::prelude::test_dht_db;
use holochain_types::activity::ChainItems;

fn options() -> holochain_p2p::event::GetOptions {
    holochain_p2p::event::GetOptions {
        follow_redirects: false,
        all_live_actions_with_metadata: true,
        request_type: Default::default(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
    let db = test_dht_db();

    let td = EntryTestData::create();

    fill_db(&db.to_db(), td.store_entry_op.clone());
    let options = options();

    let result = handle_get_entry(db.to_db().into(), td.hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&db.to_db(), td.delete_entry_action_op.clone());

    let result = handle_get_entry(db.to_db().into(), td.hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![td.wire_delete.clone()],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&db.to_db(), td.update_content_op.clone());

    let result = handle_get_entry(db.to_db().into(), td.hash.clone(), options.clone())
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
async fn get_record() {
    observability::test_run().ok();
    let db = test_dht_db();

    let td = RecordTestData::create();

    fill_db(&db.to_db(), td.store_record_op.clone());

    let options = options();

    let result = handle_get_record(db.to_db().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireRecordOps {
        action: Some(td.wire_create.clone()),
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&db.to_db(), td.deleted_by_op.clone());

    let result = handle_get_record(db.to_db().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireRecordOps {
        action: Some(td.wire_create.clone()),
        deletes: vec![td.wire_delete.clone()],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&db.to_db(), td.update_record_op.clone());

    let result = handle_get_record(db.to_db().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireRecordOps {
        action: Some(td.wire_create.clone()),
        deletes: vec![td.wire_delete.clone()],
        updates: vec![td.wire_update.clone()],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&db.to_db(), td.any_store_record_op.clone());

    let result = handle_get_record(
        db.to_db().into(),
        td.any_action_hash.clone(),
        options.clone(),
    )
    .await
    .unwrap();
    let expected = WireRecordOps {
        action: Some(td.any_action.clone()),
        deletes: vec![],
        updates: vec![],
        entry: td.any_entry.clone(),
    };
    assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn retrieve_record() {
    observability::test_run().ok();
    let db = test_dht_db();

    let td = RecordTestData::create();

    fill_db_pending(&db.to_db(), td.store_record_op.clone());

    let mut options = options();
    options.request_type = GetRequest::Pending;

    let result = handle_get_record(db.to_db().into(), td.create_hash.clone(), options.clone())
        .await
        .unwrap();
    let expected = WireRecordOps {
        action: Some(td.wire_create.clone()),
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_links() {
    observability::test_run().ok();
    let db = test_dht_db();

    let td = EntryTestData::create();

    fill_db(&db.to_db(), td.store_entry_op.clone());
    fill_db(&db.to_db(), td.create_link_op.clone());
    let options = actor::GetLinksOptions::default();

    let result = handle_get_links(db.to_db().into(), td.link_key.clone(), (&options).into())
        .await
        .unwrap();
    let expected = WireLinkOps {
        creates: vec![td.wire_create_link.clone()],
        deletes: vec![],
    };
    assert_eq!(result, expected);

    fill_db(&db.to_db(), td.delete_link_op.clone());

    let result = handle_get_links(
        db.to_db().into(),
        td.link_key_tag.clone(),
        (&options).into(),
    )
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
    let db = test_dht_db();

    let td = ActivityTestData::valid_chain_scenario();

    for hash_op in td.hash_ops.iter().cloned() {
        fill_db(&db.to_db(), hash_op);
    }
    for hash_op in td.noise_ops.iter().cloned() {
        fill_db(&db.to_db(), hash_op);
    }

    let options = actor::GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_actions: false,
        ..Default::default()
    };

    let result = handle_get_agent_activity(
        db.to_db().into(),
        td.agent.clone(),
        QueryFilter::new(),
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

    let filter = QueryFilter::new().sequence_range(ChainQueryFilterRange::ActionSeqRange(0, 19));
    let result = handle_get_agent_activity(
        db.to_db().into(),
        td.agent.clone(),
        filter,
        (&options).into(),
    )
    .await
    .unwrap();

    assert_eq!(result, expected);
}
