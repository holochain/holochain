use super::Workspace;
use crate::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use sx_state::{db::DbManager, prelude::*};

// TODO: visibility
pub struct GenesisWorkspace<'env> {
    pub source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> GenesisWorkspace<'env> {
    pub fn new(_reader: Reader<'env>, _dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for GenesisWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
