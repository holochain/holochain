use super::BufferedStore;
use super::KvBufUsed;
use super::KvOp;
use crate::buffer::kv::generic::KvStoreT;
use crate::env::ReadManager;
use crate::env::WriteManager;
use crate::error::DatabaseResult;
use crate::test_utils::test_cell_env;
use crate::test_utils::DbString;
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use rkv::StoreOptions;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::BTreeMap;
use tracing::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TestVal {
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V(pub u32);

impl From<u32> for V {
    fn from(s: u32) -> Self {
        Self(s)
    }
}

fixturator!(V; from u32;);

pub(super) type Store = KvBufUsed<DbString, V>;

macro_rules! res {
    ($key:expr, $op:ident, $val:expr) => {
        ($key, KvOp::$op(Box::new(V($val))))
    };
    ($key:expr, $op:ident) => {
        ($key, KvOp::$op)
    };
}

fn test_buf(a: &BTreeMap<Vec<u8>, KvOp<V>>, b: impl Iterator<Item = (&'static str, KvOp<V>)>) {
    for (k, v) in b {
        let val = a.get(k.as_bytes()).expect("Missing key");
        assert_eq!(*val, v);
    }
}

#[tokio::test(threaded_scheduler)]
async fn kv_iterators() -> DatabaseResult<()> {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();
        buf.put("d".into(), V(4)).unwrap();
        buf.put("e".into(), V(5)).unwrap();

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }

    env.with_reader(|reader| {
        let buf = Store::new(db);

        let forward: Vec<_> = buf
            .store()
            .iter(&reader)?
            .map(|(_, v)| Ok(v))
            .collect()
            .unwrap();
        let reverse: Vec<_> = buf
            .store()
            .iter(&reader)
            .unwrap()
            .rev()
            .map(|(_, v)| Ok(v))
            .collect()
            .unwrap();

        assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
        assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_empty_iterators() -> DatabaseResult<()> {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env
        .inner()
        .open_single("kv", StoreOptions::create())
        .unwrap();

    env.with_reader(|reader| {
        let buf = Store::new(db);

        let forward: Vec<_> = buf.store().iter(&reader).unwrap().collect().unwrap();
        let reverse: Vec<_> = buf.store().iter(&reader).unwrap().rev().collect().unwrap();

        assert_eq!(forward, vec![]);
        assert_eq!(reverse, vec![]);
        Ok(())
    })
}

/// TODO break up into smaller tests
#[tokio::test(threaded_scheduler)]
async fn kv_store_sanity_check() -> DatabaseResult<()> {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db1 = env.inner().open_single("kv1", StoreOptions::create())?;
    let db2 = env.inner().open_single("kv1", StoreOptions::create())?;

    let testval = TestVal { name: "Joe".into() };

    let mut kv1: KvBufUsed<DbString, TestVal> = KvBufUsed::new(db1);
    let mut kv2: KvBufUsed<DbString, DbString> = KvBufUsed::new(db2);

    env.with_commit(|txn| {
        kv1.put("hi".into(), testval.clone()).unwrap();
        kv2.put("salutations".into(), "folks".into()).unwrap();
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1.store().get(txn, &"hi".into())?, None);
        assert_eq!(kv2.store().get(txn, &"salutations".into())?, None);
        kv1.flush_to_txn(txn)
    })?;

    assert_eq!(kv2.scratch().len(), 1);

    env.with_commit(|txn| kv2.flush_to_txn(txn))?;

    env.with_reader(|reader| {
        // Now open some fresh Readers to see that our data was persisted
        let kv1b: KvBufUsed<DbString, TestVal> = KvBufUsed::new(db1);
        let kv2b: KvBufUsed<DbString, DbString> = KvBufUsed::new(db2);
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1b.store().get(&reader, &"hi".into())?, Some(testval));
        assert_eq!(
            kv2b.store().get(&reader, &"salutations".into())?,
            Some("folks".into())
        );
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_indicate_value_overwritten() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;
    env.with_reader(|reader| {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(1)).unwrap();
        assert_eq!(Some(V(1)), buf.get(&reader, &"a".into())?);
        buf.put("a".into(), V(2)).unwrap();
        assert_eq!(Some(V(2)), buf.get(&reader, &"a".into())?);
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_deleted_persisted() -> DatabaseResult<()> {
    use tracing::*;
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    env.with_reader(|reader| {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();
        assert!(buf.contains(&reader, &"b".into())?);

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    })?;
    env.with_reader(|reader| {
        let mut buf: KvBufUsed<DbString, V> = KvBufUsed::new(db);

        buf.delete("b".into()).unwrap();
        assert!(!buf.contains(&reader, &"b".into())?);

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    })?;
    env.with_reader(|reader| {
        let buf: KvBufUsed<DbString, _> = KvBufUsed::new(db);

        let forward = buf
            .store()
            .iter(&reader)
            .unwrap()
            .collect::<Vec<_>>()
            .unwrap();
        debug!(?forward);
        assert_eq!(forward, vec![(&b"a"[..], V(1)), (&b"c"[..], V(3))],);
        assert!(!buf.contains(&reader, &"b".into())?);
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_deleted_buffer() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(5)).unwrap();
        buf.put("b".into(), V(4)).unwrap();
        buf.put("c".into(), V(9)).unwrap();
        test_buf(
            &buf.scratch,
            [res!("a", Put, 5), res!("b", Put, 4), res!("c", Put, 9)]
                .iter()
                .cloned(),
        );
        buf.delete("b".into()).unwrap();
        test_buf(
            &buf.scratch,
            [res!("a", Put, 5), res!("c", Put, 9), res!("b", Delete)]
                .iter()
                .cloned(),
        );

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }
    env.with_reader(|reader| {
        let buf: KvBufUsed<DbString, _> = KvBufUsed::new(db);

        let forward: Vec<_> = buf.store().iter(&reader).unwrap().collect().unwrap();
        assert_eq!(forward, vec![(&b"a"[..], V(5)), (&b"c"[..], V(9))]);
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_get_buffer() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    env.with_reader(|reader| {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(5)).unwrap();
        buf.put("b".into(), V(4)).unwrap();
        buf.put("c".into(), V(9)).unwrap();
        let n = buf.get(&reader, &"b".into())?;
        assert_eq!(n, Some(V(4)));

        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_get_persisted() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }

    env.with_reader(|reader| {
        let buf = Store::new(db);

        let n = buf.get(&reader, &"b".into())?;
        assert_eq!(n, Some(V(2)));
        assert!(buf.contains(&reader, &"b".into())?);
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_get_del_buffer() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    env.with_reader(|reader| {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(5)).unwrap();
        buf.put("b".into(), V(4)).unwrap();
        buf.put("c".into(), V(9)).unwrap();
        buf.delete("b".into()).unwrap();
        let n = buf.get(&reader, &"b".into())?;
        assert_eq!(n, None);
        assert!(!buf.contains(&reader, &"b".into())?);
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_get_del_persisted() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();
    let db = env.inner().open_single("kv", StoreOptions::create())?;

    {
        let mut buf = Store::new(db);

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }

    env.with_reader(|reader| {
        let mut buf: KvBufUsed<DbString, V> = KvBufUsed::new(db);

        buf.delete("b".into()).unwrap();
        let n = buf.get(&reader, &"b".into())?;
        assert_eq!(n, None);

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    })?;

    env.with_reader(|reader| {
        let buf: KvBufUsed<DbString, V> = KvBufUsed::new(db);

        let n = buf.get(&reader, &"b".into())?;
        assert_eq!(n, None);
        Ok(())
    })
}
