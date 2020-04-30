use super::Workspace;
use crate::core::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use holochain_state::{db::DbManager, prelude::*};

// TODO: visibility
pub struct GenesisWorkspace<'env> {
    pub source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> Workspace<'env> for GenesisWorkspace<'env> {
    fn new(reader: &'env Reader<'env>, dbs: &DbManager) -> WorkspaceResult<Self> {
        Ok(Self {
            source_chain: SourceChainBuf::<'env>::new(reader, dbs)?,
        })
    }
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
