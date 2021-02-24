use super::tests::VFixturator;
use super::tests::V;
use super::BufferedStore;
use super::KvBufUsed;
use crate::buffer::kv::generic::KvStoreT;
use crate::db::ReadManager;
use crate::db::WriteManager;
use crate::error::DatabaseError;
use crate::prelude::*;
use crate::test_utils::test_cell_env;
use crate::test_utils::DbString;
use ::fixt::prelude::*;
use fallible_iterator::DoubleEndedFallibleIterator;
use fallible_iterator::FallibleIterator;
use rkv::StoreOptions;
use std::collections::BTreeMap;
use tracing::*;

pub(super) type Store = KvBufUsed<DbString, V>;

#[tokio::test(threaded_scheduler)]
async fn kv_iter_from_partial() {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();

    {
        let mut buf: Store = KvBufUsed::new(db.clone());

        buf.put("a".into(), V(101)).unwrap();
        buf.put("b".into(), V(102)).unwrap();
        buf.put("dogs_likes_7".into(), V(1)).unwrap();
        buf.put("dogs_likes_79".into(), V(2)).unwrap();
        buf.put("dogs_likes_3".into(), V(3)).unwrap();
        buf.put("dogs_likes_88".into(), V(4)).unwrap();
        buf.put("dogs_likes_f".into(), V(5)).unwrap();
        buf.put("d".into(), V(103)).unwrap();
        buf.put("e".into(), V(104)).unwrap();
        buf.put("aaaaaaaaaaaaaaaaaaaa".into(), V(105)).unwrap();
        buf.put("eeeeeeeeeeeeeeeeeeee".into(), V(106)).unwrap();

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
    }

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let buf: Store = KvBufUsed::new(db.clone());

        let iter = buf.store().iter_from(&reader, "dogs_likes".into()).unwrap();
        let results = iter.collect::<Vec<_>>().unwrap();
        assert_eq!(
            results,
            vec![
                (&b"dogs_likes_3"[..], V(3)),
                (&b"dogs_likes_7"[..], V(1)),
                (&b"dogs_likes_79"[..], V(2)),
                (&b"dogs_likes_88"[..], V(4)),
                (&b"dogs_likes_f"[..], V(5)),
                (&b"e"[..], V(104)),
                (&b"eeeeeeeeeeeeeeeeeeee"[..], V(106)),
            ]
        );

        Ok(())
    })
    .unwrap();
}

enum TestData {
    Put((DbString, V)),
    Del(DbString),
}

// Runs the poor prop test
// This generates an easy to copy and paste
// Vec of values to use for a test if a bug is found
// and prints iton failure
fn do_test<R: Readable>(
    reader: &R,
    buf: &mut Store,
    puts_dels_iter: &mut impl Iterator<Item = TestData>,
    expected_state: &mut BTreeMap<DbString, V>,
    runs: &mut Vec<String>,
    reproduce: &mut Vec<String>,
    from_key: &DbString,
) {
    let mut rng = rand::thread_rng();
    for _ in 0..rng.gen_range(1, 300) {
        match puts_dels_iter.next() {
            Some(TestData::Put((key, value))) => {
                runs.push(format!("Put: key: {}, val: {:?} -> ", key, value));
                reproduce.push(format!(
                    "TestData::Put(({:?}.to_string(), {:?})), ",
                    key, value
                ));
                buf.put(key.clone(), value.clone()).unwrap();
                expected_state.insert(key, value);
            }
            Some(TestData::Del(key)) => {
                runs.push(format!("Del: key: {} -> ", key));
                reproduce.push(format!("TestData::Del({:?}.to_string()), ", key));
                buf.delete(key.clone()).unwrap();
                expected_state.remove(&key);
            }
            None => break,
        }
    }
    // Remove any empty keys
    expected_state.remove(&"".into());
    // Check single iter
    assert_eq!(
        buf.iter(reader)
            .unwrap()
            .map(|(k, v)| Ok((DbString::from_key_bytes_or_friendly_panic(k), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}\n{}];",
        runs.concat(),
        reproduce.concat()
    );
    // Check iter from
    assert_eq!(
        buf.iter_from(reader, from_key.clone())
            .unwrap()
            .map(|(k, v)| Ok((DbString::from_key_bytes_or_friendly_panic(k), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .range::<DbString, _>(from_key..)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}\n{}];\n from_key: {:?}",
        runs.concat(),
        reproduce.concat(),
        from_key,
    );
    // Check reverse
    assert_eq!(
        buf.iter(reader)
            .unwrap()
            .rev()
            .map(|(k, v)| Ok((DbString::from_key_bytes_or_friendly_panic(k), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .iter()
            .rev()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}\n{}];",
        runs.concat(),
        reproduce.concat()
    );
}

// Runs the found bugs tests
fn re_do_test<R: Readable>(
    reader: &R,
    buf: &mut Store,
    puts_dels_iter: &mut impl Iterator<Item = TestData>,
    expected_state: &mut BTreeMap<DbString, V>,
    runs: &mut Vec<String>,
    from_key: &DbString,
) {
    while let Some(td) = puts_dels_iter.next() {
        match td {
            TestData::Put((key, value)) => {
                runs.push(format!("Put: key: {}, val: {:?} -> ", key, value));
                buf.put(key.clone(), value.clone()).unwrap();
                expected_state.insert(key, value);
            }
            TestData::Del(key) => {
                runs.push(format!("Del: key: {} -> ", key));
                buf.delete(key.clone()).unwrap();
                expected_state.remove(&key);
            }
        }
    }
    expected_state.remove(&"".into());
    assert_eq!(
        buf.iter(reader)
            .unwrap()
            .map(|(k, v)| Ok((DbString::from_key_bytes_or_friendly_panic(k), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}",
        runs.concat(),
    );
    assert_eq!(
        buf.iter_from(reader, from_key.clone())
            .unwrap()
            .map(|(k, v)| Ok((DbString::from_key_bytes_or_friendly_panic(k), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .range::<DbString, _>(from_key..)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}",
        runs.concat(),
    );
    assert_eq!(
        buf.iter(reader)
            .unwrap()
            .rev()
            .map(|(k, v)| Ok((DbString::from_key_bytes_or_friendly_panic(k), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .iter()
            .rev()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}",
        runs.concat(),
    );
}

// Poor persons proptest.
// TODO: This should probaly be a real prop test
// but I couldn't easily figure out how to generate
// expected values and integrate with our need to
// test scratch and db
#[tokio::test(threaded_scheduler)]
async fn kv_single_iter() {
    observability::test_run().ok();
    let mut rng = rand::thread_rng();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();
    let td = StringFixturator::new(Unpredictable)
        .zip(VFixturator::new(Unpredictable))
        .filter(|(k, _)| k.len() > 0)
        .map(|(k, v)| (DbString::from(k), v))
        .take(300)
        .collect::<BTreeMap<_, _>>();
    let td_vec = td.into_iter().collect::<Vec<_>>();
    let mut puts = rng
        .sample_iter(rand::distributions::Uniform::new(0, td_vec.len()))
        .map(|i| td_vec[i].clone());
    let from_key = puts.next().unwrap().0;
    let mut dels = rng
        .sample_iter(rand::distributions::Uniform::new(0, td_vec.len()))
        .map(|i| td_vec[i].0.clone());
    let puts_dels = puts
        .map(|p| {
            if rng.gen() {
                TestData::Put(p.into())
            } else {
                TestData::Del(dels.next().unwrap())
            }
        })
        .take(1000)
        .collect::<Vec<_>>();
    let mut puts_dels = puts_dels.into_iter();
    let mut expected_state: BTreeMap<DbString, V> = BTreeMap::new();

    let span = trace_span!("kv_single_iter");
    let _g = span.enter();

    let mut runs = vec!["Start | ".into()];
    let mut reproduce = vec!["\nReproduce:\n".into()];

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: Store = KvBufUsed::new(db.clone());
        let span = trace_span!("in_scratch");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("")
        ));
        reproduce.push(format!(
            "let {} = vec![",
            span.metadata().map(|f| f.name()).unwrap_or("")
        ));
        do_test(
            &reader,
            &mut buf,
            &mut puts_dels,
            &mut expected_state,
            &mut runs,
            &mut reproduce,
            &from_key,
        );
        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: Store = KvBufUsed::new(db.clone());
        let span = trace_span!("in_db_first");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("")
        ));
        reproduce.push(format!(
            "]; \n\nlet {} = vec![",
            span.metadata().map(|f| f.name()).unwrap_or("")
        ));
        do_test(
            &reader,
            &mut buf,
            &mut puts_dels,
            &mut expected_state,
            &mut runs,
            &mut reproduce,
            &from_key,
        );
        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();
    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: Store = KvBufUsed::new(db.clone());
        let span = trace_span!("in_db_second");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("")
        ));
        reproduce.push(format!(
            "]; \n\nlet {} = vec![",
            span.metadata().map(|f| f.name()).unwrap_or("")
        ));
        do_test(
            &reader,
            &mut buf,
            &mut puts_dels,
            &mut expected_state,
            &mut runs,
            &mut reproduce,
            &from_key,
        );
        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn kv_single_iter_found_1() {
    observability::test_run().ok();
    let in_scratch = vec![
        TestData::Del(".".into()),
        TestData::Put(("bar".into(), V(0))),
        TestData::Put(("!".into(), V(0))),
        TestData::Put(("💯".into(), V(889149878))),
        TestData::Put(("bing".into(), V(823748021))),
        TestData::Put(("foo".into(), V(3698192405))),
        TestData::Del("💯".into()),
        TestData::Del("bing".into()),
        TestData::Put(("baz".into(), V(3224166057))),
        TestData::Del("💩".into()),
        TestData::Put(("baz".into(), V(3224166057))),
        TestData::Put((".".into(), V(0))),
        TestData::Del("❤".into()),
        TestData::Put(("💯".into(), V(889149878))),
        TestData::Put((".".into(), V(0))),
        TestData::Del("bing".into()),
        TestData::Put(("!".into(), V(0))),
        TestData::Del("!".into()),
        TestData::Del(".".into()),
    ];
    let in_db_first = vec![
        TestData::Del("!".into()),
        TestData::Del("foo".into()),
        TestData::Del("foo".into()),
        TestData::Put(("foo".into(), V(3698192405))),
        TestData::Put(("bar".into(), V(0))),
        TestData::Del("!".into()),
        TestData::Del("💩".into()),
        TestData::Put(("bar".into(), V(0))),
    ];
    let from_key = "foo".into();
    let in_db_second = vec![];
    let span = trace_span!("kv_single_iter_found_1");
    let _g = span.enter();
    kv_single_iter_runner(
        in_scratch.into_iter(),
        in_db_first.into_iter(),
        in_db_second.into_iter(),
        from_key,
    )
    .await;
}

#[tokio::test(threaded_scheduler)]
#[should_panic]
async fn kv_single_iter_found_2() {
    observability::test_run().ok();
    let in_scratch = vec![TestData::Del("".into()), TestData::Put(("".into(), V(0)))];
    let in_db_first = vec![TestData::Del("".into()), TestData::Put(("".into(), V(2)))];
    let in_db_second = vec![TestData::Del("".into()), TestData::Put(("".into(), V(2)))];
    let from_key = "foo".into();
    let span = trace_span!("kv_single_iter_found_2");
    let _g = span.enter();
    kv_single_iter_runner(
        in_scratch.into_iter(),
        in_db_first.into_iter(),
        in_db_second.into_iter(),
        from_key,
    )
    .await;
}

#[tokio::test(threaded_scheduler)]
async fn kv_single_iter_found_3() {
    observability::test_run().ok();
    let in_scratch = vec![
        TestData::Put(("m".into(), V(0))),
        TestData::Put(("n".into(), V(0))),
        TestData::Put(("o".into(), V(0))),
        TestData::Put(("o".into(), V(0))),
        TestData::Put(("p".into(), V(0))),
    ];
    let in_db_first = vec![
        TestData::Put(("o".into(), V(2))),
        TestData::Put(("o".into(), V(2))),
        TestData::Put(("o".into(), V(2))),
    ];
    let in_db_second = vec![
        TestData::Put(("o".into(), V(2))),
        TestData::Del("o".into()),
        TestData::Put(("o".into(), V(2))),
    ];
    let from_key = "o".into();
    let span = trace_span!("kv_single_iter_found_3");
    let _g = span.enter();
    kv_single_iter_runner(
        in_scratch.into_iter(),
        in_db_first.into_iter(),
        in_db_second.into_iter(),
        from_key,
    )
    .await;
}

#[tokio::test(threaded_scheduler)]
async fn kv_single_iter_found_4() {
    observability::test_run().ok();
    let in_scratch = vec![TestData::Put((".".into(), V(0)))];

    let in_db_first = vec![
        TestData::Del("💯".into()),
        TestData::Del("foo".into()),
        TestData::Put(("bing".into(), V(2017453015))),
        TestData::Del("❤".into()),
    ];
    let from_key = "foo".into();
    let in_db_second = vec![];
    let span = trace_span!("kv_single_iter_found_4");
    let _g = span.enter();
    kv_single_iter_runner(
        in_scratch.into_iter(),
        in_db_first.into_iter(),
        in_db_second.into_iter(),
        from_key,
    )
    .await;
}

#[tokio::test(threaded_scheduler)]
async fn exhaust_both_ends() {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();
    let values = (b'a'..=b'z')
        .map(|a| DbString::from_key_bytes_or_friendly_panic(&[a]))
        .zip((0..).into_iter().map(V))
        .collect::<Vec<_>>();
    let expected = [
        (b"a", V(0)),
        (b"z", V(25)),
        (b"b", V(1)),
        (b"y", V(24)),
        (b"c", V(2)),
        (b"x", V(23)),
        (b"d", V(3)),
        (b"w", V(22)),
        (b"e", V(4)),
        (b"v", V(21)),
        (b"f", V(5)),
        (b"u", V(20)),
        (b"g", V(6)),
        (b"t", V(19)),
        (b"h", V(7)),
        (b"s", V(18)),
        (b"i", V(8)),
        (b"r", V(17)),
        (b"j", V(9)),
        (b"q", V(16)),
        (b"k", V(10)),
        (b"p", V(15)),
        (b"l", V(11)),
        (b"o", V(14)),
        (b"m", V(12)),
        (b"n", V(13)),
    ];
    let expected = expected
        .iter()
        .map(|(k, v)| ([k[0]], v.clone()))
        .collect::<Vec<_>>();
    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: Store = KvBufUsed::new(db.clone());
        for (k, v) in values {
            buf.put(k, v).unwrap();
        }
        {
            let mut i = buf.iter(&reader).unwrap().map(|(k, v)| Ok(([k[0]], v)));
            let mut result = Vec::new();
            loop {
                match (i.next().unwrap(), i.next_back().unwrap()) {
                    (Some(f), Some(b)) => {
                        result.push(f);
                        result.push(b);
                    }
                    (Some(f), None) => result.push(f),
                    (None, Some(b)) => result.push(b),
                    (None, None) => break,
                }
            }
            assert_eq!(result, expected);
        }
        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();
    env.with_reader::<DatabaseError, _, _>(|reader| {
        let buf: Store = KvBufUsed::new(db.clone());
        let mut i = buf.iter(&reader).unwrap().map(|(k, v)| Ok(([k[0]], v)));
        let mut result = Vec::new();
        loop {
            match (i.next().unwrap(), i.next_back().unwrap()) {
                (Some(f), Some(b)) => {
                    result.push(f);
                    result.push(b);
                }
                (Some(f), None) => result.push(f),
                (None, Some(b)) => result.push(b),
                (None, None) => break,
            }
        }
        assert_eq!(result, expected);
        Ok(())
    })
    .unwrap();
}

async fn kv_single_iter_runner(
    in_scratch: impl Iterator<Item = TestData> + Send,
    in_db_first: impl Iterator<Item = TestData> + Send,
    in_db_second: impl Iterator<Item = TestData> + Send,
    from_key: DbString,
) {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();

    let mut runs = vec!["Start | ".into()];
    let mut expected_state: BTreeMap<DbString, V> = BTreeMap::new();

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: Store = KvBufUsed::new(db.clone());
        let span = trace_span!("in_scratch");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("in_scratch")
        ));
        re_do_test(
            &reader,
            &mut buf,
            &mut in_scratch.into_iter(),
            &mut expected_state,
            &mut runs,
            &from_key,
        );
        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: Store = KvBufUsed::new(db.clone());
        let span = trace_span!("in_db_first");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("in_db_first")
        ));
        re_do_test(
            &reader,
            &mut buf,
            &mut in_db_first.into_iter(),
            &mut expected_state,
            &mut runs,
            &from_key,
        );
        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: Store = KvBufUsed::new(db.clone());
        let span = trace_span!("in_db_second");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("in_db_second")
        ));
        re_do_test(
            &reader,
            &mut buf,
            &mut in_db_second.into_iter(),
            &mut expected_state,
            &mut runs,
            &from_key,
        );
        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();
}
