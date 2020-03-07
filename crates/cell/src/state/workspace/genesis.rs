use super::Workspace;
use crate::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use sx_state::{db::DbManager, exports::Writer, prelude::*};

pub struct GenesisWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> GenesisWorkspace<'env> {
    pub fn new(reader: Reader<'env>, dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for GenesisWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        Ok(self.source_chain.flush_to_txn(&mut writer)?)
    }
}

#[cfg(test)]
pub mod tests {

    use super::GenesisWorkspace;
    use crate::state::source_chain::SourceChainResult;
    use sx_state::{env::ReadManager, error::DatabaseError, test_utils::test_env};

    type Err = DatabaseError;

    #[test]
    fn can_commit_workspace() -> SourceChainResult<()> {
        let env = test_env();
        let dbs = env.dbs()?;
        env.with_reader::<Err, _, _>(|reader| {
            let workspace = GenesisWorkspace::new(reader, &dbs);
            Ok(())
        })?;
        Ok(())
    }
}
