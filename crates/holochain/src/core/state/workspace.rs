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
pub trait Workspace: Send + Sized {
    /// Flush accumulated changes to the writer without committing.
    /// This consumes the Workspace.
    ///
    /// No method is provided to commit the writer as well, because Writers
    /// should be managed such that write failures are properly handled, which
    /// is outside the scope of the workspace.
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()>;
}

#[cfg(test)]
pub mod tests {

    use super::Workspace;
    use crate::core::state::workspace::WorkspaceResult;
    use holochain_state::{
        buffer::{BufferedStore, KvBufFresh},
        db::{GetDb, ELEMENT_VAULT_HEADERS, ELEMENT_VAULT_PUBLIC_ENTRIES},
        prelude::*,
        test_utils::{test_cell_env, DbString},
    };
    use holochain_types::{prelude::*, test_utils::fake_header_hash};

    pub struct TestWorkspace {
        one: KvBufFresh<HeaderHash, u32>,
        two: KvBufFresh<DbString, bool>,
    }

    impl TestWorkspace {
        pub fn new(env: EnvironmentRead, dbs: &impl GetDb) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBufFresh::new(env.clone(), dbs.get_db(&*ELEMENT_VAULT_PUBLIC_ENTRIES)?),
                two: KvBufFresh::new(env.clone(), dbs.get_db(&*ELEMENT_VAULT_HEADERS)?),
            })
        }
    }

    impl Workspace for TestWorkspace {
        fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
            self.one.flush_to_txn(writer)?;
            self.two.flush_to_txn(writer)?;
            Ok(())
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn workspace_sanity_check() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await;
        let addr1 = fake_header_hash(1);
        let addr2: DbString = "hi".into();
        {
            let _reader = env.reader()?;
            let mut workspace = TestWorkspace::new(arc.clone().into(), &dbs)?;
            assert_eq!(workspace.one.get(&addr1).await?, None);

            workspace.one.put(addr1.clone(), 1).unwrap();
            workspace.two.put(addr2.clone(), true).unwrap();
            assert_eq!(workspace.one.get(&addr1).await?, Some(1));
            assert_eq!(workspace.two.get(&addr2).await?, Some(true));
            env.with_commit(|mut writer| workspace.flush_to_txn(&mut writer))?;
        }

        // Ensure that the data was persisted
        {
            let _reader = env.reader()?;
            let workspace = TestWorkspace::new(arc.clone().into(), &dbs)?;
            assert_eq!(workspace.one.get(&addr1).await?, Some(1));
        }
        Ok(())
    }
}
