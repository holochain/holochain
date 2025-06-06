use super::*;
use crate::test_utils::*;
#[cfg(feature = "unstable-warrants")]
use holo_hash::fixt::ActionHashFixturator;
#[cfg(feature = "unstable-warrants")]
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain_p2p::actor;
use holochain_state::prelude::test_dht_db;
#[cfg(feature = "unstable-warrants")]
use {crate::authority::handle_get_agent_activity, holochain_types::activity::ChainItems};

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    holochain_trace::test_run();
    let db = test_dht_db();

    let td = EntryTestData::create();

    fill_db(&db.to_db(), td.store_entry_op.clone()).await;

    let result = handle_get_entry(db.to_db().into(), td.hash.clone())
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

    let result = handle_get_entry(db.to_db().into(), td.hash.clone())
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

    let result = handle_get_entry(db.to_db().into(), td.hash.clone())
        .await
        .unwrap();
    let result_2 = handle_get_entry(db.to_db().into(), td.hash.clone())
        .await
        .unwrap();
    let result_3 = handle_get_entry(db.to_db().into(), td.hash.clone())
        .await
        .unwrap();
    println!("wire entry ops 1 {result:?}");
    println!("wire entry ops 2 {result_2:?}");
    println!("result == result_2 {}", result == result_2);
    println!("result2 == result_3 {}", result_2 == result_3);
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

    let result = handle_get_record(db.to_db().into(), td.create_hash.clone())
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

    let result = handle_get_record(db.to_db().into(), td.create_hash.clone())
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

    let result = handle_get_record(db.to_db().into(), td.create_hash.clone())
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

    let result = handle_get_record(db.to_db().into(), td.any_action_hash.clone())
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

#[cfg(feature = "unstable-warrants")]
#[tokio::test(flavor = "multi_thread")]
async fn get_agent_activity() {
    use ::fixt::fixt;
    use holochain_state::mutations::*;

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
        td.agent.clone(),
    );
    let warrant_invalid = Warrant::new_now(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: td.agent.clone(),
            action: (fixt!(ActionHash), fixt!(Signature)),
            validation_type: ValidationType::Sys,
        }),
        fixt!(AgentPubKey),
        td.agent.clone(),
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
                insert_op_dht(txn, &op, 0, None).unwrap();
                set_validation_status(txn, &hash, ValidationStatus::Valid).unwrap();
                set_when_integrated(txn, &hash, Timestamp::now()).unwrap();
            }
            {
                let op: DhtOpHashed = warrant_op_invalid.downcast();
                let hash = op.to_hash();
                insert_op_dht(txn, &op, 0, None).unwrap();
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
