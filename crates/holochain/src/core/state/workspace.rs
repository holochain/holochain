use super::source_chain::SourceChainError;
use holochain_state::{
    db::GetDb,
    error::DatabaseError,
    prelude::{Reader, Writer},
};
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

pub trait Workspace<'env>: Send + Sized {
    // fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self>;
    fn commit_txn(self, writer: Writer) -> Result<(), WorkspaceError>;
}

#[cfg(test)]
pub mod tests {

    use super::Workspace;
    use crate::core::state::workspace::WorkspaceResult;
    use holochain_state::{
        buffer::{BufferedStore, KvBuf},
        db::{GetDb, PRIMARY_CHAIN_ENTRIES, PRIMARY_CHAIN_HEADERS},
        prelude::*,
        test_utils::test_cell_env,
    };
    use holochain_types::prelude::*;

    pub struct TestWorkspace<'env, R: Readable> {
        one: KvBuf<'env, EntryHash, u32, R>,
        two: KvBuf<'env, String, bool, R>,
    }

    impl<'env, R: Readable> TestWorkspace<'env, R> {
        pub fn new(reader: &'env R, dbs: &'env impl GetDb) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBuf::new(reader, dbs.get_db(&*PRIMARY_CHAIN_ENTRIES)?)?,
                two: KvBuf::new(reader, dbs.get_db(&*PRIMARY_CHAIN_HEADERS)?)?,
            })
        }
    }

    impl<'env, R: Readable + Send + Sync> Workspace<'env> for TestWorkspace<'env, R> {
        fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
            self.one.flush_to_txn(&mut writer)?;
            self.two.flush_to_txn(&mut writer)?;
            writer.commit()?;
            Ok(())
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn workspace_sanity_check() -> anyhow::Result<()> {
        let arc = test_cell_env().await;
        let env = arc.guard().await;
        let dbs = arc.dbs().await;
        let addr1 = EntryHash::with_data_sync("hello".as_bytes());
        let addr2 = "hi".to_string();
        env.with_commit(|txn| {
            let mut workspace = TestWorkspace::new(txn, &dbs)?;
            assert_eq!(workspace.one.get(&addr1)?, None);

            workspace.one.put(addr1.clone(), 1);
            workspace.two.put(addr2.clone(), true);
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
            assert_eq!(workspace.two.get(&addr2)?, Some(true));
            Ok(()) as Result<_, anyhow::Error>
        })?;

        // Ensure that the data was persisted
        {
            let reader = env.reader()?;
            let workspace = TestWorkspace::new(&reader, &dbs)?;
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
        }
        Ok(())
    }
}
