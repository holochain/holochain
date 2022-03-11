use chrono::Duration;
use chrono::Utc;
use fixt::prelude::*;
use holochain_sqlite::db::WriteManager;
use holochain_state::mutations;
use holochain_state::prelude::test_cell_db;
use holochain_types::dht_op::DhtOpLight;
use holochain_zome_types::fixt::*;
use holochain_zome_types::Timestamp;
use holochain_zome_types::ValidationStatus;

#[tokio::test(flavor = "multi_thread")]
async fn test_dht_op_query() {
    let test_db = test_cell_db();
    let db = test_db.db();

    // Create some integration values
    let mut expected = Vec::new();
    let mut basis = AnyDhtHashFixturator::new(Predictable);
    let now = Utc::now();
    let same_basis = basis.next().unwrap();
    let mut times = Vec::new();
    times.push(now - Duration::hours(100));
    times.push(now);
    times.push(now + Duration::hours(100));
    let times_exp = times.clone();
    let values = times.into_iter().map(|when_integrated| {
        (
            ValidationStatus::Valid,
            DhtOpLight::RegisterAgentActivity(fixt!(HeaderHash), basis.next().unwrap()),
            Timestamp::from(when_integrated),
        )
    });

    // Put them in the db
    {
        let mut dht_hash = DhtOpHashFixturator::new(Predictable);
        for (validation_status, op, when_integrated) in values {
            db.conn()
                .unwrap()
                .with_commit(|txn| mutations::insert_op())
                .unwrap();
            buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
            expected.push(value.clone());
            value.op = DhtOpLight::RegisterAgentActivity(fixt!(HeaderHash), same_basis.clone());
            buf.put(dht_hash.next().unwrap(), value.clone()).unwrap();
            expected.push(value.clone());
        }
    }

    // Check queries

    let mut conn = db.conn().unwrap();
    conn.with_reader_test(|mut reader| {
        let buf = IntegratedDhtOpsBuf::new(db.clone().into()).unwrap();
        // No filter
        let mut r = buf
            .query(&mut reader, None, None, None)
            .unwrap()
            .map(|(_, v)| Ok(v))
            .collect::<Vec<_>>()
            .unwrap();
        r.sort_by_key(|v| v.when_integrated.clone());
        assert_eq!(&mut r[..], &expected[..]);
        // From now
        let mut r = buf
            .query(&mut reader, Some(times_exp[1].clone().into()), None, None)
            .unwrap()
            .map(|(_, v)| Ok(v))
            .collect::<Vec<_>>()
            .unwrap();
        r.sort_by_key(|v| v.when_integrated.clone());
        assert!(r.contains(&expected[2]));
        assert!(r.contains(&expected[4]));
        assert!(r.contains(&expected[3]));
        assert!(r.contains(&expected[5]));
        assert_eq!(r.len(), 4);
        // From ages ago till 1hr in future
        let ages_ago = times_exp[0] - Duration::weeks(5);
        let future = times_exp[1] + Duration::hours(1);
        let mut r = buf
            .query(
                &mut reader,
                Some(ages_ago.into()),
                Some(future.into()),
                None,
            )
            .unwrap()
            .map(|(_, v)| Ok(v))
            .collect::<Vec<_>>()
            .unwrap();
        r.sort_by_key(|v| v.when_integrated.clone());

        assert!(r.contains(&expected[0]));
        assert!(r.contains(&expected[1]));
        assert!(r.contains(&expected[2]));
        assert!(r.contains(&expected[3]));
        assert_eq!(r.len(), 4);
        // Same basis
        let ages_ago = times_exp[0] - Duration::weeks(5);
        let future = times_exp[1] + Duration::hours(1);
        let mut r = buf
            .query(
                &mut reader,
                Some(ages_ago.into()),
                Some(future.into()),
                Some(ArcInterval::new(same_basis.get_loc(), 1)),
            )
            .unwrap()
            .map(|(_, v)| Ok(v))
            .collect::<Vec<_>>()
            .unwrap();
        r.sort_by_key(|v| v.when_integrated.clone());
        assert!(r.contains(&expected[1]));
        assert!(r.contains(&expected[3]));
        assert_eq!(r.len(), 2);
        // Same basis all
        let mut r = buf
            .query(
                &mut reader,
                None,
                None,
                Some(ArcInterval::new(same_basis.get_loc(), 1)),
            )
            .unwrap()
            .map(|(_, v)| Ok(v))
            .collect::<Vec<_>>()
            .unwrap();
        r.sort_by_key(|v| v.when_integrated.clone());
        assert!(r.contains(&expected[1]));
        assert!(r.contains(&expected[3]));
        assert!(r.contains(&expected[5]));
        assert_eq!(r.len(), 3);
    });
}
