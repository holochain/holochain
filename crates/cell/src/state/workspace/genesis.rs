use super::Workspace;
use crate::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use sx_state::{db::DbManager, exports::Writer, prelude::*, error::DatabaseError};

pub struct GenesisWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> GenesisWorkspace<'env> {
    pub fn new(_reader: Reader<'env>, _dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for GenesisWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit().map_err(DatabaseError::from)?;
        Ok(())
    }
}
