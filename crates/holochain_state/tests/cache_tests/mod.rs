use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use holo_hash::*;
use holochain_sqlite::prelude::*;
use holochain_sqlite::rusqlite::Transaction;
use holochain_state::validation_db::ValidationLimboStatus;
use holochain_state::{mutations, prelude::test_in_mem_db};
use holochain_types::db_cache::*;
use holochain_types::dht_op::{DhtOpLight, DhtOpType, OpOrder};
use holochain_zome_types::Create;
use holochain_zome_types::ValidationStatus;
use holochain_zome_types::{
    Action, ActionHashed, Dna, Signature, SignedActionHashed, Timestamp, NOISE,
};
use std::collections::HashMap;
use std::sync::Arc;

fn insert_action_and_op(txn: &mut Transaction, u: &mut Unstructured, action: &Action) -> DhtOpHash {
    let timestamp = Timestamp::arbitrary(u).unwrap();
    let op_order = OpOrder::new(DhtOpType::RegisterAgentActivity, timestamp);
    let any_hash: AnyDhtHash = EntryHash::arbitrary(u).unwrap().into();
    let action = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(action.clone()),
        Signature::arbitrary(u).unwrap(),
    );
    let hash = action.as_hash().clone();
    let op_hash = DhtOpHash::arbitrary(u).unwrap();
    mutations::insert_action(txn, &action).unwrap();
    mutations::insert_op_lite(
        txn,
        &DhtOpLight::RegisterAgentActivity(hash, any_hash.clone()),
        &op_hash,
        &op_order,
        &timestamp,
    )
    .unwrap();

    op_hash
}

fn set_integrated(db: &DbWrite<DbKindDht>, u: &mut Unstructured, op_hash: &DhtOpHash) {
    db.test_commit(|txn| {
        mutations::set_validation_stage(txn, op_hash, ValidationLimboStatus::Pending).unwrap();
        mutations::set_when_integrated(txn, op_hash, Timestamp::arbitrary(u).unwrap()).unwrap();
    });
}

fn set_ready_to_integrate(db: &DbWrite<DbKindDht>, op_hash: &DhtOpHash) {
    db.test_commit(|txn| {
        mutations::set_validation_stage(txn, op_hash, ValidationLimboStatus::AwaitingIntegration)
            .unwrap();
        mutations::set_validation_status(txn, op_hash, ValidationStatus::Valid).unwrap();
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
    let mut u = Unstructured::new(&NOISE);

    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| assert!(activity.is_empty())).await;

    let action = Action::Dna(Dna::arbitrary(&mut u).unwrap());
    let author = action.author().clone();
    let hash = ActionHash::with_data_sync(&action);
    let op_hash = db.test_commit(|txn| insert_action_and_op(txn, &mut u, &action));

    let cache = DhtDbQueryCache::new(db.clone().into());

    check_state(&cache, |activity| assert!(activity.is_empty())).await;

    set_ready_to_integrate(&db, &op_hash);

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

    set_integrated(&db, &mut u, &op_hash);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    let mut action = Create::arbitrary(&mut u).unwrap();
    action.prev_action = hash.clone();
    action.action_seq = 1;
    action.author = author.clone();
    let action: Action = action.into();
    let op_hash = db.test_commit(|txn| insert_action_and_op(txn, &mut u, &action));

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    set_ready_to_integrate(&db, &op_hash);

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

    set_integrated(&db, &mut u, &op_hash);

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
    let mut u = Unstructured::new(&NOISE);
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));

    let action = Action::Dna(Dna::arbitrary(&mut u).unwrap());
    let hash = ActionHash::with_data_sync(&action);
    let author = action.author().clone();

    // Create the missing action so we can get the hash.
    let mut missing_action = Create::arbitrary(&mut u).unwrap();
    missing_action.prev_action = hash;
    missing_action.action_seq = 1;
    missing_action.author = author.clone();
    let missing_action: Action = missing_action.into();
    let missing_hash = ActionHash::with_data_sync(&missing_action);

    let mut op_hashes = db.test_commit(|txn| {
        let mut op_hashes = Vec::new();
        op_hashes.push(insert_action_and_op(txn, &mut u, &action));

        let mut action = Create::arbitrary(&mut u).unwrap();
        action.prev_action = missing_hash;
        action.action_seq = 2;
        action.author = author.clone();
        let action: Action = action.into();
        op_hashes.push(insert_action_and_op(txn, &mut u, &action));
        op_hashes
    });

    set_ready_to_integrate(&db, &op_hashes[0]);

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

    set_integrated(&db, &mut u, &op_hashes[0]);
    set_ready_to_integrate(&db, &op_hashes[1]);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    op_hashes.push(db.test_commit(|txn| insert_action_and_op(txn, &mut u, &missing_action)));

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(action.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    set_ready_to_integrate(&db, &op_hashes[2]);

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
    let mut u = Unstructured::new(&NOISE);
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));

    let action = Action::Dna(Dna::arbitrary(&mut u).unwrap());
    let author = action.author().clone();
    db.test_commit(|txn| insert_action_and_op(txn, &mut u, &action));

    let cache = DhtDbQueryCache::new(db.clone().into());

    cache
        .set_activity_ready_to_integrate(&author, 0)
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
    cache.set_activity_to_integrated(&author, 0).await.unwrap();

    check_state(&cache, |activity| {
        dbg!(activity);
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    cache
        .set_activity_ready_to_integrate(&author, 1)
        .await
        .unwrap();
    cache
        .set_activity_ready_to_integrate(&author, 2)
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

    cache.set_activity_to_integrated(&author, 1).await.unwrap();

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

    cache.set_activity_to_integrated(&author, 2).await.unwrap();

    check_state(&cache, |activity| {
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, Some(2));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_set_all_integrated() {
    let mut u = Unstructured::new(&NOISE);
    let test_activity: Vec<_> = std::iter::repeat_with(|| {
        (
            Arc::new(AgentPubKey::arbitrary(&mut u).unwrap()),
            0..=(u.int_in_range(0..=u32::MAX).unwrap()),
        )
    })
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
    let mut u = Unstructured::new(&NOISE);
    let author = Arc::new(AgentPubKey::arbitrary(&mut u).unwrap());
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    cache
        .set_activity_ready_to_integrate(author.as_ref(), 3)
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
