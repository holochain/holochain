use super::tests::{TestBuf, VFixturator, V};
use super::{BufferedStore, KvBuf};
use crate::{
    env::{ReadManager, WriteManager},
    error::DatabaseError,
    test_utils::test_cell_env,
};
use fallible_iterator::FallibleIterator;
use fixt::prelude::*;
use rkv::StoreOptions;
use std::collections::BTreeMap;
use tracing::*;

#[tokio::test(threaded_scheduler)]
async fn kv_iter_from_partial() {
    let arc = test_cell_env();
    let env = arc.guard().await;
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: TestBuf = KvBuf::new(&reader, db).unwrap();

        buf.put("a", V(101)).unwrap();
        buf.put("b", V(102)).unwrap();
        buf.put("dogs_likes_7", V(1)).unwrap();
        buf.put("dogs_likes_79", V(2)).unwrap();
        buf.put("dogs_likes_3", V(3)).unwrap();
        buf.put("dogs_likes_88", V(4)).unwrap();
        buf.put("dogs_likes_f", V(5)).unwrap();
        buf.put("d", V(103)).unwrap();
        buf.put("e", V(104)).unwrap();
        buf.put("aaaaaaaaaaaaaaaaaaaa", V(105)).unwrap();
        buf.put("eeeeeeeeeeeeeeeeeeee", V(106)).unwrap();

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
            .unwrap();
        Ok(())
    })
    .unwrap();

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let buf: TestBuf = KvBuf::new(&reader, db).unwrap();

        let iter = buf.iter_raw_from("dogs_likes").unwrap();
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
    Put((String, V)),
    Del(String),
}

// Runs the poor prop test
// This generates an easy to copy and paste
// Vec of values to use for a test if a bug is found
// and prints iton failure
fn do_test(
    buf: &mut KvBuf<String, V>,
    puts_dels_iter: &mut impl Iterator<Item = TestData>,
    expected_state: &mut BTreeMap<String, V>,
    runs: &mut Vec<String>,
    reproduce: &mut Vec<String>,
    from_key: &String,
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
    expected_state.remove("");
    // Check single iter
    assert_eq!(
        buf.iter()
            .unwrap()
            .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
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
        buf.iter_from(from_key.clone())
            .unwrap()
            .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .range::<String, _>(from_key..)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}\n{}];\n from_key: {:?}",
        runs.concat(),
        reproduce.concat(),
        from_key,
    );
    // Check reverse
    assert_eq!(
        buf.iter_reverse()
            .unwrap()
            .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
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
fn re_do_test(
    buf: &mut KvBuf<String, V>,
    puts_dels_iter: &mut impl Iterator<Item = TestData>,
    expected_state: &mut BTreeMap<String, V>,
    runs: &mut Vec<String>,
    from_key: &String,
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
    expected_state.remove("");
    assert_eq!(
        buf.iter()
            .unwrap()
            .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
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
        buf.iter_from(from_key.clone())
            .unwrap()
            .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
            .inspect(|(k, v)| Ok(trace!(?k, ?v)))
            .collect::<Vec<_>>()
            .unwrap(),
        expected_state
            .range::<String, _>(from_key..)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        "{}",
        runs.concat(),
    );
    assert_eq!(
        buf.iter_reverse()
            .unwrap()
            .map(|(k, v)| Ok((String::from_utf8(k.to_vec()).unwrap(), v)))
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
    holochain_types::observability::test_run().ok();
    let mut rng = rand::thread_rng();
    let arc = test_cell_env();
    let env = arc.guard().await;
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();
    let td = StringFixturator::new(Unpredictable)
        .zip(VFixturator::new(Unpredictable))
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
                TestData::Put(p)
            } else {
                TestData::Del(dels.next().unwrap())
            }
        })
        .take(1000)
        .collect::<Vec<_>>();
    let mut puts_dels = puts_dels.into_iter();
    let mut expected_state: BTreeMap<String, V> = BTreeMap::new();

    let span = trace_span!("kv_single_iter");
    let _g = span.enter();

    let mut runs = vec!["Start | ".to_string()];
    let mut reproduce = vec!["\nReproduce:\n".to_string()];

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
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
        let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
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
        let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
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
    holochain_types::observability::test_run().ok();
    let in_scratch = vec![
        TestData::Del(".".to_string()),
        TestData::Put(("bar".to_string(), V(0))),
        TestData::Put(("!".to_string(), V(0))),
        TestData::Put(("💯".to_string(), V(889149878))),
        TestData::Put(("bing".to_string(), V(823748021))),
        TestData::Put(("foo".to_string(), V(3698192405))),
        TestData::Del("💯".to_string()),
        TestData::Del("bing".to_string()),
        TestData::Put(("baz".to_string(), V(3224166057))),
        TestData::Del("💩".to_string()),
        TestData::Put(("baz".to_string(), V(3224166057))),
        TestData::Put((".".to_string(), V(0))),
        TestData::Del("❤".to_string()),
        TestData::Put(("💯".to_string(), V(889149878))),
        TestData::Put((".".to_string(), V(0))),
        TestData::Del("bing".to_string()),
        TestData::Put(("!".to_string(), V(0))),
        TestData::Del("!".to_string()),
        TestData::Del(".".to_string()),
    ];
    let in_db_first = vec![
        TestData::Del("!".to_string()),
        TestData::Del("foo".to_string()),
        TestData::Del("foo".to_string()),
        TestData::Put(("foo".to_string(), V(3698192405))),
        TestData::Put(("bar".to_string(), V(0))),
        TestData::Del("!".to_string()),
        TestData::Del("💩".to_string()),
        TestData::Put(("bar".to_string(), V(0))),
    ];
    let from_key = "foo".to_string();
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
async fn kv_single_iter_found_2() {
    holochain_types::observability::test_run().ok();
    let in_scratch = vec![
        TestData::Del("".to_string()),
        TestData::Put(("".to_string(), V(0))),
    ];
    let in_db_first = vec![
        TestData::Del("".to_string()),
        TestData::Put(("".to_string(), V(2))),
    ];
    let in_db_second = vec![
        TestData::Del("".to_string()),
        TestData::Put(("".to_string(), V(2))),
    ];
    let from_key = "foo".to_string();
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
    holochain_types::observability::test_run().ok();
    let in_scratch = vec![
        TestData::Put(("m".to_string(), V(0))),
        TestData::Put(("n".to_string(), V(0))),
        TestData::Put(("o".to_string(), V(0))),
        TestData::Put(("o".to_string(), V(0))),
        TestData::Put(("p".to_string(), V(0))),
    ];
    let in_db_first = vec![
        TestData::Put(("o".to_string(), V(2))),
        TestData::Put(("o".to_string(), V(2))),
        TestData::Put(("o".to_string(), V(2))),
    ];
    let in_db_second = vec![
        TestData::Put(("o".to_string(), V(2))),
        TestData::Del("o".to_string()),
        TestData::Put(("o".to_string(), V(2))),
    ];
    let from_key = "o".to_string();
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
    holochain_types::observability::test_run().ok();
    let in_scratch = vec![TestData::Put((".".to_string(), V(0)))];

    let in_db_first = vec![
        TestData::Del("💯".to_string()),
        TestData::Del("foo".to_string()),
        TestData::Put(("bing".to_string(), V(2017453015))),
        TestData::Del("❤".to_string()),
    ];
    let from_key = "foo".to_string();
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

async fn kv_single_iter_runner(
    in_scratch: impl Iterator<Item = TestData> + Send,
    in_db_first: impl Iterator<Item = TestData> + Send,
    in_db_second: impl Iterator<Item = TestData> + Send,
    from_key: String,
) {
    let arc = test_cell_env();
    let env = arc.guard().await;
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();

    let mut runs = vec!["Start | ".to_string()];
    let mut expected_state: BTreeMap<String, V> = BTreeMap::new();

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
        let span = trace_span!("in_scratch");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("in_scratch")
        ));
        re_do_test(
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
        let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
        let span = trace_span!("in_db_first");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("in_db_first")
        ));
        re_do_test(
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
        let mut buf: KvBuf<String, V> = KvBuf::new(&reader, db).unwrap();
        let span = trace_span!("in_db_second");
        let _g = span.enter();
        runs.push(format!(
            "{} | ",
            span.metadata().map(|m| m.name()).unwrap_or("in_db_second")
        ));
        re_do_test(
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
