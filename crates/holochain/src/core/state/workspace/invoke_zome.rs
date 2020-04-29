use super::Workspace;
use crate::core::state::{source_chain::SourceChain, workspace::WorkspaceResult};
use holochain_state::{db::DbManager, prelude::*};
use shrinkwraprs::Shrinkwrap;

#[derive(Shrinkwrap)]
pub struct InvokeZomeWorkspace<'env> {
    source_chain: SourceChain<'env, Reader<'env>>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbs: &'env DbManager) -> WorkspaceResult<Self> {
        let source_chain = SourceChain::new(reader, dbs)?;
        Ok(InvokeZomeWorkspace { source_chain })
    }
}

impl<'env> Workspace for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.into_inner().flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}
