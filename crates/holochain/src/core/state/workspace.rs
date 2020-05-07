//! Workspaces are a simple abstraction used to stage changes during Workflow
//! execution to be persisted later
//!
//! Every Workflow has an associated Workspace type.

use super::source_chain::SourceChainError;
use holochain_state::{error::DatabaseError, prelude::Writer};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum WorkspaceError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
}

#[allow(missing_docs)]
pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

/// Defines a Workspace
pub trait Workspace<'env>: Send + Sized {
    // TODO: if we can have a generic way to create a Workspace, we can have
    // `run_workflow` automatically create one and pass it into the workflow
    // function -- this is also the case for the WorkflowTriggers
    // fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self>;

    /// Flush accumulated changes to the database. This consumes a Writer.
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

    pub struct TestWorkspace<'env> {
        one: KvBuf<'env, EntryHash, u32, Reader<'env>>,
        two: KvBuf<'env, String, bool, Reader<'env>>,
    }

    impl<'env> TestWorkspace<'env> {
        pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBuf::new(reader, dbs.get_db(&*PRIMARY_CHAIN_ENTRIES)?)?,
                two: KvBuf::new(reader, dbs.get_db(&*PRIMARY_CHAIN_HEADERS)?)?,
            })
        }
    }

    impl<'env> Workspace<'env> for TestWorkspace<'env> {
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
        {
            let reader = env.reader()?;
            let writer = env.writer_unmanaged()?;
            let mut workspace = TestWorkspace::new(&reader, &dbs)?;
            assert_eq!(workspace.one.get(&addr1)?, None);

            workspace.one.put(addr1.clone(), 1);
            workspace.two.put(addr2.clone(), true);
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
            assert_eq!(workspace.two.get(&addr2)?, Some(true));
            workspace.commit_txn(writer)?;
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
