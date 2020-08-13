#![allow(clippy::mutex_atomic)]
use super::*;
use crate::core::state::workspace::WorkspaceError;
use futures::Future;
use holochain_state::env::{EnvironmentRead, EnvironmentWrite};

#[derive(Clone)]
pub struct CallZomeWorkspaceFactory(EnvironmentRead);

impl From<EnvironmentRead> for CallZomeWorkspaceFactory {
    fn from(e: EnvironmentRead) -> Self {
        Self(e.into())
    }
}

impl From<EnvironmentWrite> for CallZomeWorkspaceFactory {
    fn from(e: EnvironmentWrite) -> Self {
        Self(e.into())
    }
}

impl CallZomeWorkspaceFactory {
    // TODO: make WorkspaceFactory trait to genericize this across all workspaces.
    // pub fn workspace<'r>(
    //     reader: &'r Reader<'r>,
    //     dbs: &impl GetDb,
    // ) -> WorkspaceResult<CallZomeWorkspace<'r>> {
    //     CallZomeWorkspace::new(reader, dbs)
    // }

    /// Useful when we need this type where we don't want to use it.
    /// It will always return None.
    pub fn null() -> Self {
        todo!()
    }

    pub async fn flush_to_txn<'a>(
        self,
        writer: &'a mut Writer<'a>,
    ) -> Result<(), error::WorkspaceFactoryError> {
        let env_ref = self.0.guard().await;
        let reader = env_ref.reader().map_err(WorkspaceError::from)?;
        let workspace = CallZomeWorkspace::new(&reader, &env_ref)?;
        workspace.flush_to_txn(writer)?;
        Ok(())
    }

    pub async fn apply_ref<
        'a,
        R: 'a,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&CallZomeWorkspace<'a>) -> Fut + 'a,
    >(
        &'a self,
        f: F,
    ) -> Result<R, error::WorkspaceFactoryError> {
        let env_ref = self.0.guard().await;
        let reader = env_ref.reader().map_err(WorkspaceError::from)?;
        let workspace = CallZomeWorkspace::new(&reader, &env_ref)?;
        Ok(f(&workspace).await)
    }

    pub async fn apply_mut<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a mut CallZomeWorkspace) -> Fut,
    >(
        &'a self,
        f: F,
    ) -> Result<R, error::WorkspaceFactoryError> {
        let env_ref = self.0.guard().await;
        let reader = env_ref.reader().map_err(WorkspaceError::from)?;
        let mut workspace = CallZomeWorkspace::new(&reader, &env_ref)?;
        Ok(f(&mut workspace).await)
    }

    pub async fn apply_owned<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(CallZomeWorkspace) -> Fut,
    >(
        &'a self,
        f: F,
    ) -> Result<R, error::WorkspaceFactoryError> {
        let env_ref = self.0.guard().await;
        let reader = env_ref.reader().map_err(WorkspaceError::from)?;
        let mut workspace = CallZomeWorkspace::new(&reader, &env_ref)?;
        Ok(f(workspace).await)
    }
}

pub mod error {
    use crate::core::{
        ribosome::error::RibosomeError, state::workspace::WorkspaceError,
        workflow::error::WorkflowError, SourceChainError,
    };
    use thiserror::Error;
    #[derive(Error, Debug)]
    pub enum WorkspaceFactoryError {
        #[error(transparent)]
        WorkspaceError(#[from] WorkspaceError),
        #[error(transparent)]
        WorkflowError(#[from] WorkflowError),
        #[error(transparent)]
        RibosomeError(#[from] RibosomeError),
        #[error(transparent)]
        SourceChainError(#[from] SourceChainError),
    }

    pub type WorkspaceFactoryResult<T> = Result<T, WorkspaceFactoryError>;
}
