use super::Workspace;
use crate::state::source_chain::SourceChainBuffer;
use sx_state::{db::DbManager, error::WorkspaceResult, prelude::*};

pub struct InvokeZomeWorkspace<'env> {
    source_chain: SourceChainBuffer<'env, Reader<'env>>,
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
    use crate::state::source_chain::{SourceChainBuffer, SourceChainResult};
    use sx_state::{
        env::ReadManager, error::WorkspaceError, prelude::Readable, test_utils::test_env,
    };

    type Err = WorkspaceError;

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
