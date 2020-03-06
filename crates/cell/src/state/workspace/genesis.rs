use crate::state::source_chain::SourceChainBuffer;
use sx_state::{db::DbManager, error::WorkspaceResult, prelude::Reader};

pub struct GenesisWorkspace<'env> {
    source_chain: SourceChainBuffer<'env, Reader<'env>>,
}

impl<'env> GenesisWorkspace<'env> {
    pub fn new(reader: Reader<'env>, dbs: &DbManager) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::GenesisWorkspace;
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
            let workspace = GenesisWorkspace::new(reader, &dbs);
            Ok(())
        })?;
        Ok(())
    }
}
