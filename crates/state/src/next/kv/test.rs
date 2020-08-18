use super::{BufferedStore, KvBuf, Op, Scratch};
use crate::{
    env::{ReadManager, WriteManager},
    error::{DatabaseError, DatabaseResult},
    test_utils::test_cell_env,
};
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use rkv::StoreOptions;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TestVal {
    name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct V(pub u32);

impl From<u32> for V {
    fn from(s: u32) -> Self {
        Self(s)
    }
}

fixturator!(V; from u32;);

#[tokio::test(threaded_scheduler)]
async fn kv_store_sanity_check() -> DatabaseResult<()> {
    let arc = test_cell_env();
    let env = arc.guard().await;
    let db1 = env.inner().open_single("kv1", StoreOptions::create())?;
    let db2 = env.inner().open_single("kv1", StoreOptions::create())?;
    let mut scratch1 = Scratch::new();
    let mut scratch2 = Scratch::new();

    let testval = TestVal {
        name: "Joe".to_owned(),
    };

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut kv1: KvBuf<String, TestVal> = KvBuf::new(&reader, db1, &mut scratch1)?;
        let mut kv2: KvBuf<String, String> = KvBuf::new(&reader, db2, &mut scratch2)?;

        env.with_commit(|writer| {
            kv1.put("hi".to_owned(), testval.clone()).unwrap();
            kv2.put("salutations".to_owned(), "folks".to_owned())
                .unwrap();
            // Check that the underlying store contains no changes yet
            assert_eq!(kv1.get_persisted(&"hi".to_owned())?, None);
            assert_eq!(kv2.get_persisted(&"salutations".to_owned())?, None);
            kv1.flush_to_txn(writer)
        })?;

        // Ensure that mid-transaction, there has still been no persistence,
        // just for kicks

        env.with_commit(|writer| {
            let kv1a: KvBuf<String, TestVal> = KvBuf::new(&reader, db1, &mut scratch1)?;
            assert_eq!(kv1a.get_persisted(&"hi".to_owned())?, None);
            kv2.flush_to_txn(writer)
        })
    })?;

    env.with_reader(|reader| {
        // Now open some fresh Readers to see that our data was persisted
        let kv1b: KvBuf<String, TestVal> = KvBuf::new(&reader, db1, &mut scratch1)?;
        let kv2b: KvBuf<String, String> = KvBuf::new(&reader, db2, &mut scratch2)?;
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1b.get_persisted(&"hi".to_owned())?, Some(testval));
        assert_eq!(
            kv2b.get_persisted(&"salutations".to_owned())?,
            Some("folks".to_owned())
        );
        Ok(())
    })
}

#[tokio::test(threaded_scheduler)]
async fn kv_store_sanity_check_2() -> DatabaseResult<()> {
    let arc = test_cell_env();
    let env = arc.guard().await;
    let db1 = env.inner().open_single("kv1", StoreOptions::create())?;
    let db2 = env.inner().open_single("kv1", StoreOptions::create())?;
    let mut scratch1 = Scratch::new();
    let mut scratch2 = Scratch::new();

    let testval = TestVal {
        name: "Joe".to_owned(),
    };

    env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut kv1: KvBuf<String, TestVal> = KvBuf::new(&reader, db1, &mut scratch1)?;
        let mut kv2: KvBuf<String, String> = KvBuf::new(&reader, db2, &mut scratch2)?;

        env.with_commit(|writer| {
            kv1.put("hi".to_owned(), testval.clone()).unwrap();
            kv2.put("salutations".to_owned(), "folks".to_owned())
                .unwrap();
            // Check that the underlying store contains no changes yet
            assert_eq!(kv1.get_persisted(&"hi".to_owned())?, None);
            assert_eq!(kv2.get_persisted(&"salutations".to_owned())?, None);
            kv1.flush_to_txn(writer)
        })?;

        assert_eq!(scratch1.len(), 0);
        assert_eq!(kv2.scratch().len(), 1);

        // Ensure that mid-transaction, there has still been no persistence,
        // just for kicks

        env.with_commit(|writer| {
            let kv1a: KvBuf<String, TestVal> = KvBuf::new(&reader, db1, &mut scratch1)?;
            assert_eq!(kv1a.get_persisted(&"hi".to_owned())?, None);
            kv2.flush_to_txn(writer)
        })?;

        assert_eq!(scratch1.len(), 0);
        assert_eq!(scratch2.len(), 0);

        Ok(())
    })?;

    env.with_reader(|reader| {
        // Now open some fresh Readers to see that our data was persisted
        let kv1b: KvBuf<String, TestVal> = KvBuf::new(&reader, db1, &mut scratch1)?;
        let kv2b: KvBuf<String, String> = KvBuf::new(&reader, db2, &mut scratch2)?;
        // Check that the underlying store contains no changes yet
        assert_eq!(kv1b.get_persisted(&"hi".to_owned())?, Some(testval));
        assert_eq!(
            kv2b.get_persisted(&"salutations".to_owned())?,
            Some("folks".to_owned())
        );
        Ok(())
    })
}

pub(super) type TestBuf<'a> = KvBuf<'a, &'a str, V>;

macro_rules! res {
    ($key:expr, $op:ident, $val:expr) => {
        ($key, Op::$op(Box::new(V($val))))
    };
    ($key:expr, $op:ident) => {
        ($key, Op::$op)
    };
}

fn test_buf(a: &BTreeMap<Vec<u8>, Op<V>>, b: impl Iterator<Item = (&'static str, Op<V>)>) {
    for (k, v) in b {
        let val = a.get(k.as_bytes()).expect("Missing key");
        assert_eq!(*val, v);
    }
}

// #[tokio::test(threaded_scheduler)]
// async fn kv_iterators() -> DatabaseResult<()> {
//     let arc = test_cell_env();
//     let env = arc.guard().await;
//     let db = env.inner().open_single("kv", StoreOptions::create())?;

//     env.with_reader::<DatabaseError, _, _>(|reader| {
//         let mut buf: TestBuf = KvBuf::new(&reader, db)?;

//         buf.put("a", V(1)).unwrap();
//         buf.put("b", V(2)).unwrap();
//         buf.put("c", V(3)).unwrap();
//         buf.put("d", V(4)).unwrap();
//         buf.put("e", V(5)).unwrap();

//         env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
//         Ok(())
//     })?;

//     env.with_reader(|reader| {
//         let buf: TestBuf = KvBuf::new(&reader, db)?;

//         let forward: Vec<_> = buf.iter_raw()?.map(|(_, v)| Ok(v)).collect().unwrap();
//         let reverse: Vec<_> = buf
//             .iter_raw_reverse()?
//             .map(|(_, v)| Ok(v))
//             .collect()
//             .unwrap();

//         assert_eq!(forward, vec![V(1), V(2), V(3), V(4), V(5)]);
//         assert_eq!(reverse, vec![V(5), V(4), V(3), V(2), V(1)]);
//         Ok(())
//     })
// }

// #[tokio::test(threaded_scheduler)]
// async fn kv_empty_iterators() -> DatabaseResult<()> {
//     let arc = test_cell_env();
//     let env = arc.guard().await;
//     let db = env
//         .inner()
//         .open_single("kv", StoreOptions::create())
//         .unwrap();

//     env.with_reader(|reader| {
//         let buf: TestBuf = KvBuf::new(&reader, db).unwrap();

//         let forward: Vec<_> = buf.iter_raw().unwrap().collect().unwrap();
//         let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect().unwrap();

//         assert_eq!(forward, vec![]);
//         assert_eq!(reverse, vec![]);
//         Ok(())
//     })
// }
