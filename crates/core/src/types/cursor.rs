use crate::shims::*;

pub trait CursorR {
    fn contains(&self, address: &Address) -> PersistenceResult<bool>;
    fn fetch(&self, address: &Address) -> PersistenceResult<Option<Content>>;
}

pub trait CursorRw: CursorR {
    fn add<C: AddressableContent>(&self, content: &C) -> PersistenceResult<()>;
    fn commit(self) -> PersistenceResult<()>;
}

/// A placeholder for a concrete cursor type, which happens to be read-write
pub struct CasCursorX;

impl CursorR for CasCursorX {
    fn contains(&self, address: &Address) -> PersistenceResult<bool> {
        unimplemented!()
    }

    fn fetch(&self, address: &Address) -> PersistenceResult<Option<Content>> {
        unimplemented!()
    }
}

impl CursorRw for CasCursorX {
    fn add<C: AddressableContent>(&self, content: &C) -> PersistenceResult<()> {
        unimplemented!()
    }

    fn commit(self) -> PersistenceResult<()> {
        unimplemented!()
    }
}
