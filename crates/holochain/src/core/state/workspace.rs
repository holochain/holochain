use super::source_chain::SourceChainError;
use holochain_state::{error::DatabaseError, prelude::Writer};
use thiserror::Error;

mod app_validation;
mod genesis;
mod invoke_zome;
pub use app_validation::AppValidationWorkspace;
pub use genesis::GenesisWorkspace;
pub use invoke_zome::InvokeZomeWorkspace;
pub use invoke_zome::raw::UnsafeInvokeZomeWorkspace;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

pub trait Workspace: Send {
    fn commit_txn(self, writer: Writer) -> Result<(), WorkspaceError>;
}

#[cfg(test)]
pub mod tests {

    use super::Workspace;
    use crate::core::state::workspace::WorkspaceResult;
    use holochain_state::{
        buffer::{BufferedStore, KvBuf},
        db::{DbManager, PRIMARY_CHAIN_ENTRIES, PRIMARY_CHAIN_HEADERS},
        env::{ReadManager, WriteManager},
        prelude::{Reader, Writer},
        test_utils::test_cell_env,
    };
    use holochain_types::prelude::*;

    pub struct TestWorkspace<'env> {
        one: KvBuf<'env, EntryHash, u32>,
        two: KvBuf<'env, String, bool>,
    }

    impl<'env> TestWorkspace<'env> {
        pub fn new(reader: &'env Reader<'env>, dbs: &'env DbManager) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBuf::new(reader, *dbs.get(&*PRIMARY_CHAIN_ENTRIES)?)?,
                two: KvBuf::new(reader, *dbs.get(&*PRIMARY_CHAIN_HEADERS)?)?,
            })
        }
    }

    impl<'env> Workspace for TestWorkspace<'env> {
        fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
            self.one.flush_to_txn(&mut writer)?;
            self.two.flush_to_txn(&mut writer)?;
            writer.commit()?;
            Ok(())
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn workspace_sanity_check() -> WorkspaceResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        let addr1 = EntryHash::with_data_sync("hello".as_bytes());
        let addr2 = "hi".to_string();
        {
            let reader = env.reader()?;
            let mut workspace = TestWorkspace::new(&reader, &dbs)?;
            assert_eq!(workspace.one.get(&addr1)?, None);

            workspace.one.put(addr1.clone(), 1);
            workspace.two.put(addr2.clone(), true);
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
            assert_eq!(workspace.two.get(&addr2)?, Some(true));
            workspace.commit_txn(env.writer()?)?;
        }

        // Ensure that the data was persisted
        {
            let reader = env.reader()?;
            let workspace = TestWorkspace::new(&reader, &dbs)?;
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
        }
        Ok(())
    }
}
