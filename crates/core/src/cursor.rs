use crate::attr;
use holochain_persistence_api::txn::{Cursor, CursorProvider};
use holochain_persistence_api::txn::CursorRw;
use holochain_persistence_lmdb::txn;
use holochain_persistence_lmdb::txn::{LmdbCursor};
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;
use sx_types::prelude::*;
use sx_types::shims::*;

// pub trait CursorProvider<A: Attribute, CR: Cursor<A>, CW: CursorRw<A>> {
//     fn reader(&self) -> CR;
//     fn writer(&self) -> CW;
// }

pub struct PersistenceManager<A: Attribute>(txn::LmdbManager<A>);

impl<A: Attribute> PersistenceManager<A> {
    pub fn new() -> Self {
        let env_path = PathBuf::new();//"/home/michael/Holo/lmdb-test");
        let staging_path_prefix = None;
        let initial_map_size = None;
        let env_flags = None;
        let staging_initial_map_size = None;
        let staging_env_flags = None;
        let lmdb = txn::new_manager(
            env_path,
            staging_path_prefix,
            initial_map_size,
            env_flags,
            staging_initial_map_size,
            staging_env_flags,
        );
        Self(lmdb)
    }

    pub fn reader(&self) -> PersistenceResult<impl Cursor<A>> {
        self.0.create_cursor()
    }

    pub fn writer(&self) -> PersistenceResult<impl CursorRw<A>> {
        self.0.create_cursor_rw()
    }
}

// pub type ChainCurs = LmdbCursor<attr::Chain>;
pub trait ChainCursor: Cursor<attr::Chain> {}
pub trait ChainCursorRw: CursorRw<attr::Chain> {}
pub type ChainPersistenceManager = PersistenceManager<attr::Chain>;

// pub trait Cursor<A: Attribute> {
//     fn contains_content(&self, address: &Address) -> PersistenceResult<bool>;
//     fn get_content(&self, address: &Address) -> PersistenceResult<Option<Content>>;
//     fn query_eav(&self, query: ()) -> PersistenceResult<BTreeSet<Eavi<A>>>;
// }

// pub trait CursorRw<A: Attribute>: Cursor<A> {
//     fn put_content<C: AddressableContent>(&self, content: &C) -> PersistenceResult<()>;
//     fn put_eavi(&self, eavi: Eavi<A>) -> PersistenceResult<Option<Eavi<A>>>;
//     fn commit(self) -> PersistenceResult<()>;
// }

// pub type ChainCursorX = CasCursorX<ChainAttribute>;

// impl<A: Attribute> CursorR<A> for CasCursorX<A> {
//     fn contains_content(&self, address: &Address) -> PersistenceResult<bool> {
//         unimplemented!()
//     }

//     fn get_content(&self, address: &Address) -> PersistenceResult<Option<Content>> {
//         unimplemented!()
//     }

//     fn query_eav(&self, query: ()) -> PersistenceResult<BTreeSet<Eavi<A>>> {
//         unimplemented!()
//     }
// }

// impl<A: Attribute> CursorRw<A> for CasCursorX<A> {
//     fn put_content<C: AddressableContent>(&self, content: &C) -> PersistenceResult<()> {
//         unimplemented!()
//     }

//     fn put_eavi(&self, eavi: Eavi<A>) -> PersistenceResult<Option<Eavi<A>>> {
//         unimplemented!()
//     }

//     fn commit(self) -> PersistenceResult<()> {
//         unimplemented!()
//     }
// }
