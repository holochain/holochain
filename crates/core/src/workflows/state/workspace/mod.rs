use super::source_chain::SourceChainError;
use sx_state::{error::DatabaseError, prelude::Writer};
use thiserror::Error;

mod app_validation;
mod genesis;
mod invoke_zome;
pub use app_validation::AppValidationWorkspace;
pub use genesis::GenesisWorkspace;
pub use invoke_zome::InvokeZomeWorkspace;

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
    use crate::workflows::state::workspace::WorkspaceResult;
    use sx_state::{
        buffer::{BufferedStore, KvBuf},
        db::{DbManager, CHAIN_ENTRIES, CHAIN_HEADERS},
        env::{ReadManager, WriteManager},
        prelude::{Reader, Writer},
        test_utils::test_env,
    };
    use sx_types::prelude::*;


    pub struct TestWorkspace<'env> {
        one: KvBuf<'env, Address, u32>,
        two: KvBuf<'env, Address, bool>,
    }

    impl<'env> TestWorkspace<'env> {
        pub fn new(reader: &'env Reader<'env>, dbs: &'env DbManager) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBuf::new(reader, *dbs.get(&*CHAIN_ENTRIES)?)?,
                two: KvBuf::new(reader, *dbs.get(&*CHAIN_HEADERS)?)?,
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

    #[tokio::test]
    async fn workspace_sanity_check() -> WorkspaceResult<()> {
        let arc = test_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        let addr1 = Address::from("hi".to_owned());
        let addr2 = Address::from("hi".to_owned());
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
