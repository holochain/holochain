//! Workspaces are a simple abstraction used to stage changes during Workflow
//! execution to be persisted later
//!
//! Every Workflow has an associated Workspace type.

use super::source_chain::SourceChainError;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::prelude::Writer;
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
    ///
    /// No method is provided to commit the writer as well, because Writers
    /// should be managed such that write failures are properly handled, which
    /// is outside the scope of the workspace.
    ///
    /// This method is provided and shouldn't need to be implemented. It is
    /// preferred to use this over `flush_to_txn_ref` since it's generally not
    /// valid to flush the same data twice.
    fn flush_to_txn(mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.flush_to_txn_ref(writer)
    }

    /// Flush accumulated changes to the writer, without consuming the Workspace
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()>;
}

#[cfg(test)]
pub mod tests {
    use super::Workspace;
    use crate::workspace::WorkspaceResult;
    use holochain_sqlite::buffer::BufferedStore;
    use holochain_sqlite::buffer::KvBufFresh;
    use holochain_sqlite::prelude::*;
    use holochain_sqlite::test_utils::test_cell_env;
    use holochain_sqlite::test_utils::DbString;
    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_header_hash;

    pub struct TestWorkspace {
        one: KvBufFresh<HeaderHash, u32>,
        two: KvBufFresh<DbString, bool>,
    }

    impl TestWorkspace {
        pub fn new(env: DbRead) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBufFresh::new(
                    env.clone(),
                    env.get_table(TableName::ElementVaultPublicEntries)?,
                ),
                two: KvBufFresh::new(env.clone(), env.get_table(TableName::ElementVaultHeaders)?),
            })
        }
    }

    impl Workspace for TestWorkspace {
        fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
            self.one.flush_to_txn_ref(writer)?;
            self.two.flush_to_txn_ref(writer)?;
            Ok(())
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn workspace_sanity_check() -> anyhow::Result<()> {
        let test_env = test_cell_env();
        let arc = test_env.env();
        let addr1 = fake_header_hash(1);
        let addr2: DbString = "hi".into();
        {
            let mut workspace = TestWorkspace::new(arc.clone().into())?;
            assert_eq!(workspace.one.get(&addr1)?, None);

            workspace.one.put(addr1.clone(), 1).unwrap();
            workspace.two.put(addr2.clone(), true).unwrap();
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
            assert_eq!(workspace.two.get(&addr2)?, Some(true));
            arc.conn()
                .unwrap()
                .with_commit(|mut writer| workspace.flush_to_txn(&mut writer))?;
        }

        // Ensure that the data was persisted
        {
            let workspace = TestWorkspace::new(arc.clone().into())?;
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
        }
        Ok(())
    }
}
