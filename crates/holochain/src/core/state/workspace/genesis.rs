use super::Workspace;
use crate::core::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use holochain_state::prelude::*;

// TODO: visibility
pub struct GenesisWorkspace<'env> {
    pub source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> GenesisWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbs: &'env impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self {
            source_chain: SourceChainBuf::new(reader, dbs)?,
        })
    }
}

impl<'env> Workspace for GenesisWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
