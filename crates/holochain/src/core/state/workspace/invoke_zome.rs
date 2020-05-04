use super::Workspace;
use crate::core::state::{
    source_chain::{SourceChain},
    workspace::WorkspaceResult,
};
use holochain_state::{db::DbManager, prelude::*};

pub struct InvokeZomeWorkspace<'env>
{
    pub source_chain: SourceChain<'env, Reader<'env>>,
}

impl<'env> InvokeZomeWorkspace<'env>
{
    pub fn new(reader: &'env Reader<'env>, dbs: &'env DbManager) -> WorkspaceResult<Self> {
        let source_chain = SourceChain::new(reader, dbs)?;

        /* FIXME: How do we create the cascade without creating a conflict
        with the source_chain mut borrow that happens in the ribosom `invoke_zome` call?
        // Create the cascade
        let cache = SourceChainBuf::cache(reader, dbs)?;
        // Create a cache and a cas for store and meta
        let primary_meta = ChainMetaBuf::primary(reader, dbs)?;
        let cache_meta = ChainMetaBuf::cache(reader, dbs)?;
        let cascade = Cascade::new(
            &source_chain.cas(),
            &primary_meta,
            &cache.cas(),
            &cache_meta,
        );
        */

        Ok(InvokeZomeWorkspace {
            source_chain,
        })
    }
}

impl<'env> Workspace for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.into_inner().flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
