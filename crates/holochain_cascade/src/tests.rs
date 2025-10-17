use super::*;
use ::fixt::fixt;
use holo_hash::{fixt::ActionHashFixturator, HashableContentExtSync};
use holochain_cascade::CascadeImpl;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::query::map_sql_dht_op;
use holochain_types::dht_op::{ChainOp, DhtOp};
use std::sync::Arc;
use test_utils::create_test_chain_op;

#[tokio::test]
async fn test_mock_cascade_with_records() {
    use ::fixt::fixt;
    let records = vec![fixt!(Record), fixt!(Record), fixt!(Record)];
    let cascade = MockCascade::with_records(records.clone());
    let opts = NetworkGetOptions::default();
    let (r0, _) = cascade
        .retrieve(records[0].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r1, _) = cascade
        .retrieve(records[1].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r2, _) = cascade
        .retrieve(records[2].action_address().clone().into(), opts)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(records, vec![r0, r1, r2]);
}

#[tokio::test(flavor = "multi_thread")]
async fn no_peers_returns_none() {
    let cache = test_cache_db();
    let mut network = MockHolochainP2pDnaT::new();

    let action_hash = fixt!(ActionHash);
    let op_type = ChainOpType::StoreRecord;

    // Set up the mock network to return NoPeersForLocation error
    let action_hash_2 = action_hash.clone();
    let action_hash_3 = action_hash.clone();
    network
        .expect_get_by_op_type()
        .return_once(move |action_hash_param, op_type_param| {
            assert_eq!(
                action_hash_param, action_hash_2,
                "get_by_op_type called with unexpected action hash"
            );
            assert_eq!(
                op_type_param, op_type,
                "get_by_op_type called with unexpected op type"
            );
            Err(HolochainP2pError::NoPeersForLocation(
                "".to_string(),
                action_hash_3.get_loc(),
            ))
        });
    let network = Arc::new(network);
    let cascade = CascadeImpl::empty()
        .with_cache(cache.to_db())
        .with_network(network.clone(), test_cache_db().to_db());

    // Call the method
    let result = cascade
        .fetch_op_by_type(action_hash, op_type)
        .await
        .unwrap();

    // Verify that None is returned when there are no peers
    assert!(result.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn network_error_returns_error() {
    let cache = test_cache_db();
    let mut network = MockHolochainP2pDnaT::new();

    let action_hash = fixt!(ActionHash);
    let op_type = ChainOpType::StoreRecord;

    // Set up the mock network to return a generic network error
    let action_hash_2 = action_hash.clone();
    network
        .expect_get_by_op_type()
        .return_once(move |action_hash_param, op_type_param| {
            assert_eq!(
                action_hash_param, action_hash_2,
                "get_by_op_type called with unexpected action hash"
            );
            assert_eq!(
                op_type_param, op_type,
                "get_by_op_type called with unexpected op type"
            );
            Err(HolochainP2pError::invalid_p2p_message("ohono".to_string()))
        });
    let network = Arc::new(network);
    let cascade = CascadeImpl::empty()
        .with_cache(cache.to_db())
        .with_network(network.clone(), test_cache_db().to_db());

    // Call the method and expect an error
    let result = cascade.fetch_op_by_type(action_hash, op_type).await;

    // Verify that an error is returned
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn no_network_returns_none() {
    let cache = test_cache_db();
    let cascade = CascadeImpl::empty().with_cache(cache.to_db());

    let action_hash = fixt!(ActionHash);
    let op_type = ChainOpType::StoreRecord;

    // Call without a network
    let result = cascade.fetch_op_by_type(action_hash, op_type).await;

    // Verify that None is returned if no network is present.
    assert!(matches!(result, Ok(None)));
}

#[tokio::test(flavor = "multi_thread")]
async fn fetched_op_is_added_to_cache() {
    // Test all ChainOpType variants
    let op_types = vec![
        ChainOpType::StoreRecord,
        ChainOpType::StoreEntry,
        ChainOpType::RegisterAgentActivity,
        ChainOpType::RegisterUpdatedContent,
        ChainOpType::RegisterUpdatedRecord,
        ChainOpType::RegisterDeletedBy,
        ChainOpType::RegisterDeletedEntryAction,
        ChainOpType::RegisterAddLink,
        ChainOpType::RegisterRemoveLink,
    ];

    for op_type in op_types {
        let cache = test_cache_db();
        let action_hash = fixt!(ActionHash); // Create a new hash for each test
        let mut network = MockHolochainP2pDnaT::new(); // Create a new mock for each test

        // Create appropriate test data based on op_type
        let chain_op = create_test_chain_op(op_type);
        let validation_status = ValidationStatus::Valid;

        // Set up the mock network
        let action_hash_2 = action_hash.clone();
        let chain_op_2 = chain_op.clone();
        network
            .expect_get_by_op_type()
            .return_once(move |action_hash_param, op_type_param| {
                assert_eq!(
                    action_hash_param, action_hash_2,
                    "get_by_op_type called with unexpected action hash"
                );
                assert_eq!(
                    op_type_param, op_type,
                    "get_by_op_type called with unexpected op type"
                );
                Ok(Some(WireOpByType(Judged {
                    data: chain_op_2,
                    status: Some(validation_status),
                })))
            });
        let network = Arc::new(network);
        let cascade = CascadeImpl::empty()
            .with_cache(cache.to_db())
            .with_network(network.clone(), cache.to_db());

        // Call the method
        let result = cascade
            .fetch_op_by_type(action_hash, op_type)
            .await
            .unwrap();

        // Verify the result
        assert!(result.is_some(), "Failed for op_type: {op_type:?}");
        let validated_chain_op = result.unwrap();
        // Check returned op
        assert_eq!(
            validated_chain_op.into_data(),
            chain_op,
            "Failed for op_type: {op_type:?}"
        );
        // Check chain op has been added to cache
        let dht_ops = read_ops_from_cache(&cache, validation_status);
        assert_eq!(dht_ops.len(), 1);
        assert_eq!(dht_ops[0], DhtOp::ChainOp(Box::new(chain_op)));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn different_validation_statuses() {
    let cache = test_cache_db();

    let validation_statuses = vec![
        Some(ValidationStatus::Valid),
        Some(ValidationStatus::Rejected),
        Some(ValidationStatus::Abandoned),
        None,
    ];

    for validation_status in validation_statuses {
        let mut network = MockHolochainP2pDnaT::new();

        // Create a test ChainOp
        let chain_op =
            ChainOp::StoreRecord(fixt!(Signature), fixt!(Action), RecordEntry::NotStored);
        let action_hash = chain_op.action().to_hash();
        let op_type = chain_op.get_type();

        // Set up the mock network
        let action_hash_2 = action_hash.clone();
        network
            .expect_get_by_op_type()
            .return_once(move |action_hash_param, op_type_param| {
                assert_eq!(
                    action_hash_param, action_hash_2,
                    "get_by_op_type called with unexpected action hash"
                );
                assert_eq!(
                    op_type_param, op_type,
                    "get_by_op_type called with unexpected op type"
                );
                Ok(Some(WireOpByType(Judged {
                    data: chain_op,
                    status: validation_status,
                })))
            });

        let network = Arc::new(network);
        let cascade = CascadeImpl::empty()
            .with_cache(cache.to_db())
            .with_network(network.clone(), test_cache_db().to_db());

        // Call the method
        let result = cascade
            .fetch_op_by_type(action_hash, op_type)
            .await
            .unwrap();

        // Verify the result
        assert!(
            result.is_some(),
            "Failed for validation status: {validation_status:?}",
        );
        let returned_op = result.unwrap();
        assert_eq!(returned_op.data.get_type(), ChainOpType::StoreRecord);
        assert_eq!(returned_op.status, validation_status);
    }
}

fn read_ops_from_cache(
    cache: &DbRead<DbKindCache>,
    expected_validation_status: ValidationStatus,
) -> Vec<DhtOp> {
    cache.test_read(move |txn| {
        let mut stmt = txn
            .prepare(
                "
            SELECT
                DhtOp.type AS type,
                DhtOp.validation_status AS validation_status,
                Action.blob AS action_blob,
                Entry.blob AS entry_blob
            FROM
                DhtOp
                JOIN Action ON DhtOp.action_hash = Action.hash
                    LEFT JOIN Entry ON Action.entry_hash = Entry.hash
            ",
            )
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut dht_ops = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            let dht_op = map_sql_dht_op(false, "type", row).unwrap();
            let validation_status = row.get::<_, ValidationStatus>("validation_status").unwrap();
            // Validation status should match
            assert_eq!(validation_status, expected_validation_status);
            dht_ops.push(dht_op);
        }
        dht_ops
    })
}
