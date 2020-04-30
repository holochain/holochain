use super::Workspace;
use crate::core::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use holochain_state::{db::DbManager, prelude::*};

pub struct InvokeZomeWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}


impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
    fn new(_reader: &'env Reader<'env>, _dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
