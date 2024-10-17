use super::*;
use crate::authority::handle_get_agent_activity;
use crate::test_utils::*;
use ::fixt::fixt;
use holochain_p2p::actor;
use holochain_p2p::event::GetRequest;
use holochain_state::prelude::*;
use holochain_state::prelude::{set_withhold_publish, test_dht_db};
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
    holochain_trace::test_run();
    let db = test_dht_db();

    let td = EntryTestData::create();

    fill_db(&db.to_db(), td.store_entry_op.clone()).await;
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

    fill_db(&db.to_db(), td.delete_entry_action_op.clone()).await;

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

    fill_db(&db.to_db(), td.update_content_op.clone()).await;

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
    holochain_trace::test_run();
    let db = test_dht_db();

    let td = RecordTestData::create();

    fill_db(&db.to_db(), td.store_record_op.clone()).await;

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

    fill_db(&db.to_db(), td.deleted_by_op.clone()).await;

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

    fill_db(&db.to_db(), td.update_record_op.clone()).await;

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

    fill_db(&db.to_db(), td.any_store_record_op.clone()).await;

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
    holochain_trace::test_run();
    let db = test_dht_db();

    let td = RecordTestData::create();

    fill_db_pending(&db.to_db(), td.store_record_op.clone()).await;

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
    holochain_trace::test_run();
    let db = test_dht_db();

    let td = EntryTestData::create();

    fill_db(&db.to_db(), td.store_entry_op.clone()).await;
    fill_db(&db.to_db(), td.create_link_op.clone()).await;
    let options = actor::GetLinksOptions::default();

    let result = handle_get_links(db.to_db().into(), td.link_key.clone(), (&options).into())
        .await
        .unwrap();
    let expected = WireLinkOps {
        creates: vec![td.wire_create_link.clone()],
        deletes: vec![],
    };
    assert_eq!(result, expected);

    fill_db(&db.to_db(), td.delete_link_op.clone()).await;

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
    holochain_trace::test_run();
    let db = test_dht_db();

    let td = ActivityTestData::valid_chain_scenario(false);

    for hash_op in td.agent_activity_ops.iter().cloned() {
        fill_db(&db.to_db(), hash_op).await;
    }
    for hash_op in td.noise_agent_activity_ops.iter().cloned() {
        fill_db(&db.to_db(), hash_op).await;
    }

    let warrant_valid = Warrant::new_now(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: td.agent.clone(),
            action: (fixt!(ActionHash), fixt!(Signature)),
            validation_type: ValidationType::Sys,
        }),
        fixt!(AgentPubKey),
    );
    let warrant_invalid = Warrant::new_now(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: td.agent.clone(),
            action: (fixt!(ActionHash), fixt!(Signature)),
            validation_type: ValidationType::Sys,
        }),
        fixt!(AgentPubKey),
    );
    {
        let warrant_op_valid =
            WarrantOp::from(SignedWarrant::new(warrant_valid.clone(), fixt!(Signature)))
                .into_hashed();

        let warrant_op_invalid = WarrantOp::from(SignedWarrant::new(
            warrant_invalid.clone(),
            fixt!(Signature),
        ))
        .into_hashed();

        db.write_async(move |txn| {
            {
                let op: DhtOpHashed = warrant_op_valid.downcast();
                let hash = op.to_hash();
                insert_op(txn, &op).unwrap();
                set_validation_status(txn, &hash, ValidationStatus::Valid).unwrap();
                set_when_integrated(txn, &hash, Timestamp::now()).unwrap();
            }
            {
                let op: DhtOpHashed = warrant_op_invalid.downcast();
                let hash = op.to_hash();
                insert_op(txn, &op).unwrap();
                set_validation_status(txn, &hash, ValidationStatus::Rejected).unwrap();
                set_when_integrated(txn, &hash, Timestamp::now()).unwrap();
            }
            holochain_sqlite::error::DatabaseResult::Ok(())
        })
        .await
        .unwrap();
    }

    let options = actor::GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: false,
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
        warrants: vec![warrant_valid],
        status: ChainStatus::Valid(td.chain_head.clone()),
        highest_observed: Some(td.highest_observed.clone()),
    };
    pretty_assertions::assert_eq!(result, expected);

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

    pretty_assertions::assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_agent_activity_respects_withhold_publish() {
    holochain_trace::test_run();
    let db = test_dht_db();

    let td = ActivityTestData::valid_chain_scenario(false);

    for hash_op in td.agent_activity_ops.iter().cloned() {
        fill_db(&db.to_db(), hash_op).await;
    }
    // Mark the most recent valid op as withheld
    let last_op_hash = td.agent_activity_ops.last().unwrap().hash.clone();
    db.write_async(move |txn| set_withhold_publish(txn, &last_op_hash))
        .await
        .unwrap();

    let options = actor::GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: false,
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

    let hashes_without_head = match &td.valid_hashes {
        ChainItems::Hashes(hashes) => hashes.iter().take(hashes.len() - 1).cloned().collect(),
        _ => unreachable!(),
    };
    let chain_head = match &td.valid_hashes {
        ChainItems::Hashes(hashes) => hashes.iter().nth_back(1).cloned().unwrap(),
        _ => unreachable!(),
    };

    let expected = AgentActivityResponse {
        agent: td.agent.clone(),
        valid_activity: ChainItems::Hashes(hashes_without_head),
        rejected_activity: ChainItems::NotRequested,
        warrants: Vec::with_capacity(0),
        status: ChainStatus::Valid(ChainHead {
            action_seq: chain_head.0,
            hash: chain_head.1.clone(),
        }),
        highest_observed: Some(HighestObserved {
            action_seq: chain_head.0,
            hash: vec![chain_head.1.clone()],
        }),
    };
    pretty_assertions::assert_eq!(result, expected);

    let options = actor::GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
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

    let records_without_head = match &td.valid_records {
        ChainItems::Full(records) => records
            .clone()
            .into_iter()
            .take(records.len() - 1)
            .map(|mut r| {
                if let RecordEntry::Present(_) = r.entry {
                    r.entry = RecordEntry::NotStored;
                }
                r
            })
            .collect(),
        _ => unreachable!(),
    };

    let expected = AgentActivityResponse {
        agent: td.agent.clone(),
        valid_activity: ChainItems::Full(records_without_head),
        rejected_activity: ChainItems::NotRequested,
        warrants: Vec::with_capacity(0),
        status: ChainStatus::Valid(ChainHead {
            action_seq: chain_head.0,
            hash: chain_head.1.clone(),
        }),
        highest_observed: Some(HighestObserved {
            action_seq: chain_head.0,
            hash: vec![chain_head.1.clone()],
        }),
    };
    pretty_assertions::assert_eq!(result, expected);
}
