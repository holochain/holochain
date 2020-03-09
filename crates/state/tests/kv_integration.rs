
use sx_state::buffer::{BufferedStore, KvBuf};
use sx_state::{
    env::{ReadManager, WriteManager},
    error::{DatabaseError, DatabaseResult},
};
use rkv::StoreOptions;
use serde_derive::{Deserialize, Serialize};
use test_utils::test_env;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TestVal {
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct V(u32);

type TestBuf<'a> = KvBuf<'a, &'a str, V>;

#[test]
fn kv_iterators() -> DatabaseResult<()> {
    let env = test_env();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut buf: TestBuf = KvBuf::new(&reader, db)?;

        buf.put("a", V(1));
        buf.put("b", V(2));
        buf.put("c", V(3));
        buf.put("d", V(4));
        buf.put("e", V(5));

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        Ok(())
    })?;

    env.with_reader(|reader| {
        let buf: TestBuf = KvBuf::new(&reader, db)?;

        let forward: Vec<_> = buf.iter_raw()?.map(|(_, v)| v).collect();
        let reverse: Vec<_> = buf.iter_raw_reverse()?.map(|(_, v)| v).collect();

        assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
        assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
        Ok(())
    })
}

#[test]
fn kv_empty_iterators() -> DatabaseResult<()> {
    let env = test_env();
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();

    env.with_reader(|reader| {
        let buf: TestBuf = KvBuf::new(&reader, db).unwrap();

        let forward: Vec<_> = buf.iter_raw().unwrap().collect();
        let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

        assert_eq!(forward, vec![]);
        assert_eq!(reverse, vec![]);
        Ok(())
    })
}

/// TODO break up into smaller tests
#[test]
fn kv_store_sanity_check() -> DatabaseResult<()> {
    let env = test_env();
    let db1 = env.inner().open_single("kv1", StoreOptions::create())?;
    let db2 = env.inner().open_single("kv1", StoreOptions::create())?;
    let mut writer = env.writer()?;

    let testval = TestVal {
        name: "Joe".to_owned(),
    };

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut kv1: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
        let mut kv2: KvBuf<String, String> = KvBuf::new(&reader, db2)?;

        kv1.put("hi".to_owned(), testval.clone());
        kv2.put("salutations".to_owned(), "folks".to_owned());

        // Check that the underlying store contains no changes yet
        assert_eq!(kv1.get_persisted(&"hi".to_owned())?, None);
        assert_eq!(kv2.get_persisted(&"salutations".to_owned())?, None);
        kv1.flush_to_txn(&mut writer)?;

        // Ensure that mid-transaction, there has still been no persistence,
        // just for kicks
        let kv1a: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
        assert_eq!(kv1a.get_persisted(&"hi".to_owned())?, None);
        kv2.flush_to_txn(&mut writer)?;
        Ok(())
    })?;

    // Finish finalizing the transaction
    writer.commit()?;

    env.with_reader(|reader| {
        // Now open some fresh Readers to see that our data was persisted
        let kv1b: KvBuf<String, TestVal> = KvBuf::new(&reader, db1)?;
        let kv2b: KvBuf<String, String> = KvBuf::new(&reader, db2)?;
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1b.get_persisted(&"hi".to_owned())?, Some(testval));
        assert_eq!(
            kv2b.get_persisted(&"salutations".to_owned())?,
            Some("folks".to_owned())
        );
        Ok(())
    })
}
