use std::collections::BTreeSet;
use sx_types::prelude::*;
use sx_types::shims::*;

// pub struct CursorManagerX<A: Attribute>;
pub struct ChainCursorManagerX;

impl ChainCursorManagerX {
    pub fn reader(&self) -> ChainCursorX {
        CasCursorX::<SourceChainAttribute>(SourceChainAttribute::Todo)
    }

    pub fn writer(&self) -> ChainCursorX {
        CasCursorX::<SourceChainAttribute>(SourceChainAttribute::Todo)
    }
}

pub trait CursorR<A: Attribute> {
    fn contains_content(&self, address: &Address) -> PersistenceResult<bool>;
    fn get_content(&self, address: &Address) -> PersistenceResult<Option<Content>>;
    fn query_eav(&self, query: ()) -> PersistenceResult<BTreeSet<Eavi<A>>>;
}

pub trait CursorRw<A: Attribute>: CursorR<A> {
    fn put_content<C: AddressableContent>(&self, content: &C) -> PersistenceResult<()>;
    fn put_eavi(&self, eavi: Eavi<A>) -> PersistenceResult<Option<Eavi<A>>>;
    fn commit(self) -> PersistenceResult<()>;
}

/// A placeholder for a concrete cursor type, which happens to be read-write
/// this would actually contain an lmdb cursor
pub struct CasCursorX<A: Attribute>(A);

pub type ChainCursorX = CasCursorX<SourceChainAttribute>;

impl<A: Attribute> CursorR<A> for CasCursorX<A> {
    fn contains_content(&self, address: &Address) -> PersistenceResult<bool> {
        unimplemented!()
    }

    fn get_content(&self, address: &Address) -> PersistenceResult<Option<Content>> {
        unimplemented!()
    }

    fn query_eav(&self, query: ()) -> PersistenceResult<BTreeSet<Eavi<A>>> {
        unimplemented!()
    }
}

impl<A: Attribute> CursorRw<A> for CasCursorX<A> {
    fn put_content<C: AddressableContent>(&self, content: &C) -> PersistenceResult<()> {
        unimplemented!()
    }

    fn put_eavi(&self, eavi: Eavi<A>) -> PersistenceResult<Option<Eavi<A>>> {
        unimplemented!()
    }

    fn commit(self) -> PersistenceResult<()> {
        unimplemented!()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Hash, Clone, serde::Serialize, Debug)]
pub enum SourceChainAttribute {
    Todo
}
impl Attribute for SourceChainAttribute {}
