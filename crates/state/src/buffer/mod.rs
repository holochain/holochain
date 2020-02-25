
use rkv::Writer;
use crate::error::WorkspaceResult;

pub mod cas;
pub mod kv;
pub mod kvv;

/// General trait for transactional stores, exposing only the method which
/// finalizes the transaction. Not currently used, but could be used in Workspaces
/// i.e. iterating over a Vec<dyn TransactionalStore> is all that needs to happen
/// to commit the workspace changes
pub trait TransactionalStore<'env> {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()>;
}
