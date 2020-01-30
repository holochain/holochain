#[allow(unused_imports)]
use holochain_persistence_api::{eav::Attribute, error::*, txn::{CursorDyn, CursorProviderDyn, Cursor, CursorProvider}};

#[derive(Clone, Debug)]
pub struct CascadingCursorProvider<A:Attribute> {
    backing_cursor_providers : Vec<Box<dyn CursorProviderDyn<A>>>
}

#[derive(Clone, Debug)]
pub struct CascadingCursor<A:Attribute> {
    backing_cursors : Vec<Box<dyn CursorDyn<A>>>
}


impl<A:Attribtue> Cursor<A> for CascadingCursor<A:Attribute> {


}
impl<A:Attribute> CursorProvider<A> for CascadingCursorProvider<A> {
    type Cursor = CascadingCursor<A>;

    fn create_cursor(&self) -> PersistenceResult<Self::Cursor> {
       let backing_cursors = self.backing_cursor_providers.map(|cp| Box::new(cp.create_cursor()));
       backing_cursors
    }
}



