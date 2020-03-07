use super::Workspace;
use crate::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use sx_state::{db::DbManager, prelude::*};

pub struct InvokeZomeWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: Reader<'env>, dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, _writer: Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::InvokeZomeWorkspace;
    use crate::state::source_chain::SourceChainResult;
    use sx_state::{env::ReadManager, error::DatabaseError, test_utils::test_env};

    type Err = DatabaseError;

    #[test]
    fn can_commit_workspace() -> SourceChainResult<()> {
        let env = test_env();
        let dbs = env.dbs()?;
        env.with_reader::<Err, _, _>(|reader| {
            let workspace = InvokeZomeWorkspace::new(reader, &dbs);
            Ok(())
        })?;
        Ok(())
    }
}
