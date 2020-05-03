use super::Workspace;
use crate::core::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use holochain_state::prelude::*;

pub struct InvokeZomeWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(_reader: &Reader<'env>, _dbs: &impl GetDb) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}
impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
