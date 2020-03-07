use super::Workspace;

use crate::state::workspace::WorkspaceResult;
use sx_state::{db::DbManager, error::DatabaseResult, prelude::*};

pub struct AppValidationWorkspace {}

impl<'env> AppValidationWorkspace {
    pub fn new(reader: Reader<'env>, dbs: &DbManager) -> DatabaseResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for AppValidationWorkspace {
    fn commit_txn(self, _writer: Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::AppValidationWorkspace;
    use crate::state::source_chain::SourceChainResult;
    use sx_state::{env::ReadManager, error::DatabaseError, test_utils::test_env};

    type Err = DatabaseError;

    #[test]
    fn can_commit_workspace() -> SourceChainResult<()> {
        let env = test_env();
        let dbs = env.dbs()?;
        env.with_reader::<Err, _, _>(|reader| {
            let workspace = AppValidationWorkspace::new(reader, &dbs);
            Ok(())
        })?;
        Ok(())
    }
}
