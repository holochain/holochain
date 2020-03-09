
use sx_state::buffer::{BufferedStore, IntKvBuf};
use sx_state::{
    env::{Environment, ReadManager, WriteManager},
    error::DatabaseResult,
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

type Store<'a> = IntKvBuf<'a, u32, V>;

#[test]
fn kv_iterators() -> DatabaseResult<()> {
    let env: Environment = test_env();
    let db = env.inner().open_integer("kv", StoreOptions::create())?;

    env.with_reader(|reader| {
        let mut buf: Store = IntKvBuf::new(&reader, db)?;

        buf.put(1, V(1));
        buf.put(2, V(2));
        buf.put(3, V(3));
        buf.put(4, V(4));
        buf.put(5, V(5));

        env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
    })?;

    env.with_reader(|reader| {
        let buf: Store = IntKvBuf::new(&reader, db)?;

        let forward: Vec<_> = buf.iter_raw()?.collect();
        let reverse: Vec<_> = buf.iter_raw_reverse()?.collect();

        assert_eq!(
            forward,
            vec![(1, V(1)), (2, V(2)), (3, V(3)), (4, V(4)), (5, V(5))]
        );
        assert_eq!(
            reverse,
            vec![(5, V(5)), (4, V(4)), (3, V(3)), (2, V(2)), (1, V(1))]
        );
        Ok(())
    })
}

#[test]
fn kv_empty_iterators() -> DatabaseResult<()> {
    let env = test_env();
    let db = env
        .inner()
        .open_integer("kv", StoreOptions::create())
        .unwrap();

    env.with_reader(|reader| {
        let buf: Store = IntKvBuf::new(&reader, db).unwrap();

        let forward: Vec<_> = buf.iter_raw().unwrap().collect();
        let reverse: Vec<_> = buf.iter_raw_reverse().unwrap().collect();

        assert_eq!(forward, vec![]);
        assert_eq!(reverse, vec![]);
        Ok(())
    })
}
