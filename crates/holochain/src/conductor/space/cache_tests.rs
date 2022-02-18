use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_state::validation_db::ValidationLimboStatus;
use holochain_state::{mutations, prelude::test_in_mem_db};
use holochain_types::dht_op::{DhtOpLight, OpOrder};
use holochain_zome_types::Create;
use holochain_zome_types::ValidationStatus;
use holochain_zome_types::{Dna, Header, HeaderHashed, Signature, SignedHeaderHashed, NOISE};
use rusqlite::Transaction;
use test_case::test_case;

use super::*;

fn insert_header_and_op(txn: &mut Transaction, u: &mut Unstructured, header: &Header) -> DhtOpHash {
    let timestamp = Timestamp::arbitrary(u).unwrap();
    let op_order = OpOrder::new(DhtOpType::RegisterAgentActivity, timestamp);
    let any_hash: AnyDhtHash = EntryHash::arbitrary(u).unwrap().into();
    let header = SignedHeaderHashed::with_presigned(
        HeaderHashed::from_content_sync(header.clone()),
        Signature::arbitrary(u).unwrap(),
    );
    let hash = header.as_hash().clone();
    let op_hash = DhtOpHash::arbitrary(u).unwrap();
    mutations::insert_header(txn, &header).unwrap();
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
    f: impl FnOnce(&HashMap<Arc<AgentPubKey>, ActivityBounds>),
) {
    cache
        .get_or_try_init()
        .await
        .unwrap()
        .share_ref(|activity| f(activity));
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_inits_correctly() {
    let mut u = Unstructured::new(&NOISE);

    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));
    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| assert!(activity.is_empty())).await;

    let header = Header::Dna(Dna::arbitrary(&mut u).unwrap());
    let author = header.author().clone();
    let hash = HeaderHash::with_data_sync(&header);
    let op_hash = db.test_commit(|txn| insert_header_and_op(txn, &mut u, &header));

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, None);
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    set_ready_to_integrate(&db, &op_hash);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, None);
        assert_eq!(b.ready_to_integrate, Some(0));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *header.author());
    assert_eq!(to_integrate[0].1, 0..=0);

    set_integrated(&db, &mut u, &op_hash);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    let mut header = Create::arbitrary(&mut u).unwrap();
    header.prev_header = hash.clone();
    header.header_seq = 1;
    header.author = author.clone();
    let header: Header = header.into();
    let op_hash = db.test_commit(|txn| insert_header_and_op(txn, &mut u, &header));

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    set_ready_to_integrate(&db, &op_hash);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, Some(1));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *header.author());
    assert_eq!(to_integrate[0].1, 1..=1);

    set_integrated(&db, &mut u, &op_hash);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
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

    let header = Header::Dna(Dna::arbitrary(&mut u).unwrap());
    let hash = HeaderHash::with_data_sync(&header);
    let author = header.author().clone();

    // Create the missing header so we can get the hash.
    let mut missing_header = Create::arbitrary(&mut u).unwrap();
    missing_header.prev_header = hash;
    missing_header.header_seq = 1;
    missing_header.author = author.clone();
    let missing_header: Header = missing_header.into();
    let missing_hash = HeaderHash::with_data_sync(&missing_header);

    let mut op_hashes = db.test_commit(|txn| {
        let mut op_hashes = Vec::new();
        op_hashes.push(insert_header_and_op(txn, &mut u, &header));

        let mut header = Create::arbitrary(&mut u).unwrap();
        header.prev_header = missing_hash;
        header.header_seq = 2;
        header.author = author.clone();
        let header: Header = header.into();
        op_hashes.push(insert_header_and_op(txn, &mut u, &header));
        op_hashes
    });

    set_ready_to_integrate(&db, &op_hashes[0]);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, None);
        assert_eq!(b.ready_to_integrate, Some(0));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *header.author());
    assert_eq!(to_integrate[0].1, 0..=0);

    set_integrated(&db, &mut u, &op_hashes[0]);
    set_ready_to_integrate(&db, &op_hashes[1]);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    op_hashes.push(db.test_commit(|txn| insert_header_and_op(txn, &mut u, &missing_header)));

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 0);

    set_ready_to_integrate(&db, &op_hashes[2]);

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(header.author()).unwrap();
        assert_eq!(b.integrated, Some(0));
        assert_eq!(b.ready_to_integrate, Some(2));
    })
    .await;

    let to_integrate = cache.get_activity_to_integrate().await.unwrap();
    assert_eq!(to_integrate.len(), 1);
    assert_eq!(*to_integrate[0].0, *header.author());
    assert_eq!(to_integrate[0].1, 1..=2);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_set_integrated() {
    let mut u = Unstructured::new(&NOISE);
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_32(vec![0; 32]))));

    let header = Header::Dna(Dna::arbitrary(&mut u).unwrap());
    let author = header.author().clone();
    db.test_commit(|txn| insert_header_and_op(txn, &mut u, &header));

    let cache = DhtDbQueryCache::new(db.clone().into());
    check_state(&cache, |activity| {
        let b = activity.get(&author).unwrap();
        assert_eq!(b.integrated, None);
        assert_eq!(b.ready_to_integrate, None);
    })
    .await;

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

    cache.set_activity_to_integrated(&author, 0).await.unwrap();

    check_state(&cache, |activity| {
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

#[test_case(1)]
#[test_case(2)]
#[test_case(u32::MAX)]
#[test_case(u32::MAX - 1)]
fn prev_is_empty_new_is_zero_check_empty(n: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    new_bounds.integrated = Some(n);
    // () -> (n)
    assert!(!prev_is_empty_new_is_zero(None, &new_bounds));
    // (()) -> (n)
    assert!(!prev_is_empty_new_is_zero(Some(&prev_bounds), &new_bounds));
    prev_bounds.integrated = Some(5);
    // (a) -> (n)
    assert!(prev_is_empty_new_is_zero(Some(&prev_bounds), &new_bounds));
}
#[test]
fn prev_is_empty_new_is_zero_check_zero() {
    let prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    new_bounds.integrated = Some(0);
    // () -> (0)
    assert!(prev_is_empty_new_is_zero(None, &new_bounds));
    // (()) -> (0)
    assert!(prev_is_empty_new_is_zero(Some(&prev_bounds), &new_bounds));
}

#[test_case(0)]
#[test_case(1)]
#[test_case(u32::MAX - 1)]
fn integrated_is_consecutive_check_n(n: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    prev_bounds.integrated = Some(n);
    new_bounds.integrated = Some(n + 1);
    // (n) -> (n+1)
    assert!(integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
    prev_bounds.integrated = None;
    // () -> (n)
    assert!(integrated_is_consecutive(None, &new_bounds));
    // (()) -> (n)
    assert!(integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
}

#[test_case(0)]
#[test_case(1)]
#[test_case(u32::MAX)]
fn integrated_is_consecutive_check_empty(n: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let new_bounds = ActivityBounds::default();
    // (n) -> ()
    prev_bounds.integrated = Some(n);
    assert!(integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
}

#[test_case(0, 2)]
#[test_case(0, u32::MAX)]
#[test_case(1, 3)]
#[test_case(1, u32::MAX)]
#[test_case(u32::MAX - 2, u32::MAX)]
#[test_case(u32::MAX, 0)]
#[test_case(u32::MAX - 1, 0)]
#[test_case(1, 0)]
#[test_case(2, 0)]
#[test_case(2, 1)]
#[test_case(3, 1)]
fn integrated_is_consecutive_check_finds_gaps(s: u32, e: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    // (s) -> (e)
    prev_bounds.integrated = Some(s);
    new_bounds.integrated = Some(e);
    assert!(!integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
}

#[test]
fn can_accept_ready_in_random_order() {
    use rand::prelude::*;
    let mut activity = HashMap::new();
    let mut u = Unstructured::new(&NOISE);
    let mut rand = rand::thread_rng();
    let test_activity: Vec<_> = std::iter::repeat_with(|| {
        let mut seq: Vec<_> = (0..=50).collect();
        seq.shuffle(&mut rand);
        (AgentPubKey::arbitrary(&mut u).unwrap(), seq)
    })
    .take(100)
    .collect();

    let mut new_bounds = ActivityBounds::default();
    for (author, sequence) in test_activity {
        let mut spent = Vec::with_capacity(sequence.len());
        for n in sequence {
            spent.push(n);
            spent.sort_unstable();
            new_bounds.ready_to_integrate = Some(n);
            update_activity(&mut activity, &author, &new_bounds).unwrap();
            let current_top = spent
                .iter()
                .zip(spent.iter().skip(1))
                .find(|(a, b)| **b != **a + 1)
                .map(|(a, _)| *a)
                .unwrap_or_else(|| *spent.last().unwrap());
            assert_eq!(
                activity.get(&author).unwrap().ready_to_integrate.unwrap(),
                current_top
            );
        }
    }
}

fn show_query(txn: &mut Transaction) {
    let r: Vec<Result<(AgentPubKey, u32, Option<bool>, Option<bool>), _>> = txn
        .prepare(
            "
SELECT
  HA.author,
  MIN(HA.seq) min_seq,
  HA.hash IN (
      SELECT * FROM (
              SELECT
                header_hash
              FROM
                DhtOp
              WHERE
                DhtOp.type = :register_activity
                AND DhtOp.validation_stage = 3
                AND DhtOp.validation_status IS NOT NULL
      )
  ),
  HB.hash NOT IN (
      SELECT * FROM (
              SELECT
                header_hash
              FROM
                DhtOp
              WHERE
                DhtOp.type = :register_activity
                AND DhtOp.validation_stage = 3
                AND DhtOp.validation_status IS NOT NULL
      )
  )
FROM
  Header AS HA
  LEFT JOIN HEADER HB ON HB.prev_hash = HA.hash
GROUP BY
  HA.author
        ",
        )
        .unwrap()
        .query_map(
            named_params! { ":register_activity": DhtOpType::RegisterAgentActivity},
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap()
        .collect();
    dbg!(r);
}
