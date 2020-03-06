use crate::state::source_chain::SourceChainBuffer;
use sx_state::{db::DbManager, error::WorkspaceResult, prelude::Reader};

pub struct GenesisWorkspace<'env> {
    source_chain: SourceChainBuffer<'env, Reader<'env>>,
    // meta: KvvBuffer<'env, String, String>,
}

impl<'env> GenesisWorkspace<'env> {
    pub fn new(reader: Reader<'env>, dbs: DbManager<'env>) -> WorkspaceResult<Self> {
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
    fn can_perform_genesis() -> SourceChainResult<()> {
        let arc = test_env();
        let env = arc.env();
        let dbs = arc.dbs()?;
        env.with_reader::<Err, _, _>(|reader| {
            let workspace = GenesisWorkspace::new(reader, dbs);
            Ok(())
        })?;
        Ok(())
    }
}
