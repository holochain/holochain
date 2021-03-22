use super::BufferedStore;
use super::KvBufUsed;
use super::KvOp;
use crate::buffer::kv::generic::KvStoreT;
use crate::error::DatabaseResult;
use crate::prelude::*;
use crate::test_utils::test_cell_db;
use crate::test_utils::DbString;
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
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

#[tokio::test(flavor = "multi_thread")]
async fn kv_iterators() -> DatabaseResult<()> {
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;

    {
        let mut buf = Store::new(table.clone());

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();
        buf.put("d".into(), V(4)).unwrap();
        buf.put("e".into(), V(5)).unwrap();

        db.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }

    db.conn().unwrap().with_reader(|mut reader| {
        let buf = Store::new(table.clone());

        let forward: Vec<_> = buf
            .store()
            .iter(&mut reader)?
            .map(|(_, v)| Ok(v))
            .collect()
            .unwrap();
        let reverse: Vec<_> = buf
            .store()
            .iter(&mut reader)
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

#[tokio::test(flavor = "multi_thread")]
async fn kv_empty_iterators() -> DatabaseResult<()> {
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv").unwrap();

    db.conn().unwrap().with_reader(|mut reader| {
        let buf = Store::new(table.clone());

        let forward: Vec<_> = buf.store().iter(&mut reader).unwrap().collect().unwrap();
        let reverse: Vec<_> = buf
            .store()
            .iter(&mut reader)
            .unwrap()
            .rev()
            .collect()
            .unwrap();

        assert_eq!(forward, vec![]);
        assert_eq!(reverse, vec![]);
        Ok(())
    })
}

/// TODO break up into smaller tests
#[tokio::test(flavor = "multi_thread")]
async fn kv_store_sanity_check() -> DatabaseResult<()> {
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let db1 = conn.open_single("kv1")?;
    let db2 = conn.open_single("kv1")?;

    let testval = TestVal { name: "Joe".into() };

    let mut kv1: KvBufUsed<DbString, TestVal> = KvBufUsed::new(db1.clone());
    let mut kv2: KvBufUsed<DbString, DbString> = KvBufUsed::new(db2.clone());

    db.conn().unwrap().with_commit(|txn| {
        kv1.put("hi".into(), testval.clone()).unwrap();
        kv2.put("salutations".into(), "folks".into()).unwrap();
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1.store().get(txn, &"hi".into())?, None);
        assert_eq!(kv2.store().get(txn, &"salutations".into())?, None);
        kv1.flush_to_txn(txn)
    })?;

    assert_eq!(kv2.scratch().len(), 1);

    db.conn()
        .unwrap()
        .with_commit(|txn| kv2.flush_to_txn(txn))?;

    db.conn().unwrap().with_reader(|mut reader| {
        // Now open some fresh Readers to see that our data was persisted
        let kv1b: KvBufUsed<DbString, TestVal> = KvBufUsed::new(db1.clone());
        let kv2b: KvBufUsed<DbString, DbString> = KvBufUsed::new(db2.clone());
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1b.store().get(&mut reader, &"hi".into())?, Some(testval));
        assert_eq!(
            kv2b.store().get(&mut reader, &"salutations".into())?,
            Some("folks".into())
        );
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn kv_indicate_value_overwritten() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;
    db.conn().unwrap().with_reader(|mut reader| {
        let mut buf = Store::new(table.clone());

        buf.put("a".into(), V(1)).unwrap();
        assert_eq!(Some(V(1)), buf.get(&mut reader, &"a".into())?);
        buf.put("a".into(), V(2)).unwrap();
        assert_eq!(Some(V(2)), buf.get(&mut reader, &"a".into())?);
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn kv_deleted_persisted() -> DatabaseResult<()> {
    use tracing::*;
    observability::test_run().ok();
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;

    db.conn().unwrap().with_reader(|mut reader| {
        let mut buf = Store::new(table.clone());

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();
        assert!(buf.contains(&mut reader, &"b".into())?);

        db.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    })?;
    db.conn().unwrap().with_reader(|mut reader| {
        let mut buf: KvBufUsed<DbString, V> = KvBufUsed::new(table.clone());

        buf.delete("b".into()).unwrap();
        assert!(!buf.contains(&mut reader, &"b".into())?);

        db.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    })?;
    db.conn().unwrap().with_reader(|mut reader| {
        let buf: KvBufUsed<DbString, _> = KvBufUsed::new(table.clone());

        let forward = buf
            .store()
            .iter(&mut reader)
            .unwrap()
            .collect::<Vec<_>>()
            .unwrap();
        debug!(?forward);
        assert_eq!(forward, vec![(b"a".to_vec(), V(1)), (b"c".to_vec(), V(3))],);
        assert!(!buf.contains(&mut reader, &"b".into())?);
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn kv_deleted_buffer() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;

    {
        let mut buf = Store::new(table.clone());

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

        db.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }
    db.conn().unwrap().with_reader(|mut reader| {
        let buf: KvBufUsed<DbString, _> = KvBufUsed::new(table.clone());

        let forward: Vec<_> = buf.store().iter(&mut reader).unwrap().collect().unwrap();
        assert_eq!(forward, vec![(b"a".to_vec(), V(5)), (b"c".to_vec(), V(9))]);
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn kv_get_buffer() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;

    db.conn().unwrap().with_reader(|mut reader| {
        let mut buf = Store::new(table.clone());

        buf.put("a".into(), V(5)).unwrap();
        buf.put("b".into(), V(4)).unwrap();
        buf.put("c".into(), V(9)).unwrap();
        let n = buf.get(&mut reader, &"b".into())?;
        assert_eq!(n, Some(V(4)));

        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn kv_get_persisted() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;

    {
        let mut buf = Store::new(table.clone());

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();

        db.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }

    db.conn().unwrap().with_reader(|mut reader| {
        let buf = Store::new(table.clone());

        let n = buf.get(&mut reader, &"b".into())?;
        assert_eq!(n, Some(V(2)));
        assert!(buf.contains(&mut reader, &"b".into())?);
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn kv_get_del_buffer() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;

    db.conn().unwrap().with_reader(|mut reader| {
        let mut buf = Store::new(table.clone());

        buf.put("a".into(), V(5)).unwrap();
        buf.put("b".into(), V(4)).unwrap();
        buf.put("c".into(), V(9)).unwrap();
        buf.delete("b".into()).unwrap();
        let n = buf.get(&mut reader, &"b".into())?;
        assert_eq!(n, None);
        assert!(!buf.contains(&mut reader, &"b".into())?);
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn kv_get_del_persisted() -> DatabaseResult<()> {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let db = test_db.db();
    let mut conn = db.conn().unwrap();
    let table = conn.open_single("kv")?;

    {
        let mut buf = Store::new(table.clone());

        buf.put("a".into(), V(1)).unwrap();
        buf.put("b".into(), V(2)).unwrap();
        buf.put("c".into(), V(3)).unwrap();

        db.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
    }

    db.conn().unwrap().with_reader(|mut reader| {
        let mut buf: KvBufUsed<DbString, V> = KvBufUsed::new(table.clone());

        buf.delete("b".into()).unwrap();
        let n = buf.get(&mut reader, &"b".into())?;
        assert_eq!(n, None);

        db.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    })?;

    db.conn().unwrap().with_reader(|mut reader| {
        let buf: KvBufUsed<DbString, V> = KvBufUsed::new(table.clone());

        let n = buf.get(&mut reader, &"b".into())?;
        assert_eq!(n, None);
        Ok(())
    })
}
