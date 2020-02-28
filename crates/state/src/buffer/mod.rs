
use rkv::Writer;
use crate::error::WorkspaceResult;

mod cas;
mod chain_cas;
mod kv;
mod kv_int;
mod kvv;

pub use cas::CasBuffer;
pub use chain_cas::ChainCasBuffer;
pub use kv::KvBuffer;
pub use kv_int::KvIntBuffer;
pub use kvv::KvvBuffer;

/// General trait for transactional stores, exposing only the method which
/// finalizes the transaction. Not currently used, but could be used in Workspaces
/// i.e. iterating over a Vec<dyn StoreBuffer> is all that needs to happen
/// to commit the workspace changes
pub trait StoreBuffer<'env, K, V> {
    // fn iter(&self) -> WorkspaceResult<Box<dyn Iterator<Item=(V)> + 'env>>;
    // fn iter_reverse(&self) -> WorkspaceResult<Box<dyn Iterator<Item=(V)> + 'env>>;
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()>;
}
