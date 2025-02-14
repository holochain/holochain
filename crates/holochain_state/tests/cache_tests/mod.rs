use ::fixt::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DhtOpHashFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holo_hash::*;
use holochain_sqlite::prelude::*;
use holochain_state::mutations;
use holochain_state::prelude::*;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::Arc;
use test_case::test_case;

fn insert_action_and_op(txn: &mut Txn<DbKindDht>, action: &Action) -> DhtOpHash {
    let timestamp = Timestamp::now();
    let op_order = OpOrder::new(ChainOpType::RegisterAgentActivity, timestamp);
    let basis_hash: OpBasis = fixt!(EntryHash).into();
    let action = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(action.clone()),
        Signature(vec![1; 64].try_into().unwrap()),
    );
    let hash = action.as_hash().clone();
    let op_hash = fixt!(DhtOpHash);
    mutations::insert_action(txn, &action).unwrap();
    mutations::insert_op_lite(
        txn,
        &ChainOpLite::RegisterAgentActivity(hash, basis_hash.clone()).into(),
        &op_hash,
        &op_order,
        &timestamp,
        0,
        None,
    )
    .unwrap();

    op_hash
}

fn set_integrated(db: &DbWrite<DbKindDht>, op_hash: DhtOpHash) {
    db.test_write({
        let op_hash = op_hash.clone();
        move |txn| {
            mutations::set_validation_stage(txn, &op_hash, ValidationStage::Pending).unwrap();
            mutations::set_when_integrated(txn, &op_hash, Timestamp::now()).unwrap();
        }
    });
}

fn set_ready_to_integrate(db: &DbWrite<DbKindDht>, op_hash: DhtOpHash) {
    db.test_write(move |txn| {
        mutations::set_validation_stage(txn, &op_hash, ValidationStage::AwaitingIntegration)
            .unwrap();
        mutations::set_validation_status(txn, &op_hash, ValidationStatus::Valid).unwrap();
    });
}

async fn check_state(
    cache: &DhtDbQueryCache,
    f: impl FnOnce(&HashMap<Arc<AgentPubKey>, ActivityState>),
) {
    cache.get_state().await.share_ref(|activity| f(activity));
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_inits_correctly() {
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| assert!(activity.is_empty())).await;

    let action = Action::Dna(Dna {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now(),
        hash: fixt!(DnaHash),
    });
    let author = action.author().clone();
    let hash = ActionHash::with_data_sync(&action);
    let op_hash = db.test_write({
        let action = action.clone();
        move |txn| insert_action_and_op(txn, &action)
    });

    let cache = DhtDbQueryCache::new(db.clone().into());

    check_state(&cache, |activity| assert!(activity.is_empty())).await;

    set_ready_to_integrate(&db, op_hash.clone());

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, None);
        assert_eq!(b.ready_to_integrate, Some(0));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *action.author());
    assert_eq!(to_integrate[0].1, 0..=0);

    set_integrated(&db, op_hash.clone());

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    let mut action = fixt!(Create);
    action.prev_action = hash.clone();
    action.action_seq = 1;
    action.author = author.clone();
    let action: Action = action.into();
    let op_hash = db.test_write({
        let action = action.clone();
        move |txn| insert_action_and_op(txn, &action)
    });

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    set_ready_to_integrate(&db, op_hash.clone());

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, Some(1));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *action.author());
    assert_eq!(to_integrate[0].1, 1..=1);

    set_integrated(&db, op_hash.clone());

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(1));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_init_catches_gaps() {
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));

    let action = Action::Dna(Dna {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now(),
        hash: fixt!(DnaHash),
    });
    let hash = ActionHash::with_data_sync(&action);
    let author = action.author().clone();

    // Create the missing action so we can get the hash.
    let mut missing_action = fixt!(Create);
    missing_action.prev_action = hash;
    missing_action.action_seq = 1;
    missing_action.author = author.clone();
    let missing_action: Action = missing_action.into();
    let missing_hash = ActionHash::with_data_sync(&missing_action);

    let mut op_hashes = db.test_write({
        let action = action.clone();
        move |txn| {
            let mut op_hashes = Vec::new();
            op_hashes.push(insert_action_and_op(txn, &action));

            let mut action = fixt!(Create);
            action.prev_action = missing_hash;
            action.action_seq = 2;
            action.author = author.clone();
            let action: Action = action.into();
            op_hashes.push(insert_action_and_op(txn, &action));
            op_hashes
        }
    });

    set_ready_to_integrate(&db, op_hashes[0].clone());

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, {
        let author = action.author().clone();
        move |activity| {
            let b = activity.get(&author).unwrap();
            assert_eq!(b.integrated, None);
            assert_eq!(b.ready_to_integrate, Some(0));
        }
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *action.author());
    assert_eq!(to_integrate[0].1, 0..=0);

    set_integrated(&db, op_hashes[0].clone());
    set_ready_to_integrate(&db, op_hashes[1].clone());

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    op_hashes.push(db.test_write(move |txn| insert_action_and_op(txn, &missing_action)));

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    set_ready_to_integrate(&db, op_hashes[2].clone());

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, Some(2));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *action.author());
    assert_eq!(to_integrate[0].1, 1..=2);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_set_integrated() {
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));

    let action = Action::Dna(fixt!(Dna));
    let author = action.author().clone();
    db.test_write(move |txn| insert_action_and_op(txn, &action));

    let cache = DhtDbQueryCache::new(db.clone().into());

    cache
        .set_activity_ready_to_integrate(&author, Some(0))
        .await
        .unwrap();

    check_state(&cache, |activity| {
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, None);
        assert_eq!(b.ready_to_integrate, Some(0));
    })
    .await;

    check_state(&cache, |activity| {
        dbg!(activity);
    })
    .await;
    cache
        .set_activity_to_integrated(&author, Some(0))
        .await
        .unwrap();

    check_state(&cache, |activity| {
        dbg!(activity);
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    cache
        .set_activity_ready_to_integrate(&author, Some(1))
        .await
        .unwrap();
    cache
        .set_activity_ready_to_integrate(&author, Some(2))
        .await
        .unwrap();

    check_state(&cache, |activity| {
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, Some(2));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, author);
    assert_eq!(to_integrate[0].1, 1..=2);

    cache
        .set_activity_to_integrated(&author, Some(1))
        .await
        .unwrap();

    check_state(&cache, |activity| {
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, Some(1));
        assert_eq!(b.ready_to_integrate, Some(2));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, author);
    assert_eq!(to_integrate[0].1, 2..=2);

    cache
        .set_activity_to_integrated(&author, Some(2))
        .await
        .unwrap();

    check_state(&cache, |activity| {
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, Some(2));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_set_all_integrated() {
    let test_activity: Vec<_> = std::iter::repeat_with(|| (Arc::new(fixt!(AgentPubKey)), 0..=100))
        .take(1000)
        .collect();
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    cache
        .set_all_activity_to_integrated(test_activity.clone())
        .await
        .unwrap();
    check_state(&cache, |activity| {
        for (author, seq_range) in &test_activity {
            let b = activity.get(author.as_ref()).unwrap();
            assert_eq!(b.integrated, Some(*seq_range.end()));
            assert_eq!(b.ready_to_integrate, None);
        }
    })
    .await;
    let test_activity: HashMap<_, _> = test_activity.into_iter().collect();
    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    for (author, seq_range) in to_integrate {
        let range = test_activity.get(&author).unwrap();
        assert_eq!(*range, seq_range);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn check_none_integrated_with_awaiting_deps() {
    let author = Arc::new(fixt!(AgentPubKey));
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    cache
        .set_activity_ready_to_integrate(author.as_ref(), Some(3))
        .await
        .unwrap();
    check_state(&cache, |activity| {
        let b = activity.get(author.as_ref()).unwrap();
        assert_eq!(b.integrated, None);
        assert_eq!(b.ready_to_integrate, None);
        assert_eq!(b.awaiting_deps, vec![3]);
    })
    .await;
    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert!(to_integrate.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn no_activities_to_integrate_when_nothing_waiting() {
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();

    assert!(to_integrate.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn no_activities_to_integrate_when_everything_already_integrated() {
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    let agent_key = Arc::new(fixt!(AgentPubKey));

    // Set some actions to be integrated
    for i in 0..10 {
        cache
            .set_activity_to_integrated(&agent_key, Some(i))
            .await
            .unwrap();
    }

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();

    assert!(to_integrate.is_empty());
}

#[test_case(0, 1, 0..=0 ; "only first action at index 0")]
#[test_case(0, 8, 0..=7 ; "starting from index 0")]
#[test_case(13, 8, 13..=20 ; "starting from later index")]
#[test_case(20, 1, 20..=20 ; "more integrated actions than ready to integrate ones")]
#[tokio::test(flavor = "multi_thread")]
// Pass expected as parameter so can use `pretty_assertions::assert_eq`
async fn range_of_activities_to_integrate_for_single_agent(
    start: u32,
    action_count: u32,
    expected_range: RangeInclusive<u32>,
) {
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    let agent_key = Arc::new(fixt!(AgentPubKey));

    // Set previous actions to integrated
    for i in 0..start {
        cache
            .set_activity_to_integrated(&agent_key, Some(i))
            .await
            .unwrap();
    }

    for i in start..start + action_count {
        cache
            .set_activity_ready_to_integrate(&agent_key, Some(i))
            .await
            .unwrap();
    }

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();

    assert_eq!(to_integrate.len(), 1);
    assert_eq!(to_integrate[0].0, agent_key);
    assert_eq!(to_integrate[0].1, expected_range);
}
