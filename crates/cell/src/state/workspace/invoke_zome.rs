use super::Workspace;
use crate::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use sx_state::{db::DbManager, prelude::*};

pub struct InvokeZomeWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(_reader: Reader<'env>, _dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
