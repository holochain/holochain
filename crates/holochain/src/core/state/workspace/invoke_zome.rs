use super::Workspace;
use crate::core::state::{
    cascade::Cascade, chain_cas::ChainCasBuf, chain_meta::ChainMetaBuf, source_chain::SourceChain,
    workspace::WorkspaceResult,
};
use holochain_state::{db::DbManager, prelude::*};

pub struct InvokeZomeWorkspace<'env> {
    pub source_chain: SourceChain<'env>,
    pub meta: ChainMetaBuf<'env>,
    pub cache_cas: ChainCasBuf<'env>,
    pub cache_meta: ChainMetaBuf<'env>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbs: &'env DbManager) -> WorkspaceResult<Self> {
        let source_chain = SourceChain::new(reader, dbs)?;

        let cache_cas = ChainCasBuf::cache(reader, dbs)?;
        let meta = ChainMetaBuf::primary(reader, dbs)?;
        let cache_meta = ChainMetaBuf::cache(reader, dbs)?;

        Ok(InvokeZomeWorkspace {
            source_chain,
            meta,
            cache_cas,
            cache_meta,
        })
    }

    pub fn cascade(&self) -> Cascade {
        Cascade::new(
            &self.source_chain.cas(),
            &self.meta,
            &self.cache_cas,
            &self.cache_meta,
        )
    }
}

impl<'env> Workspace for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.into_inner().flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
