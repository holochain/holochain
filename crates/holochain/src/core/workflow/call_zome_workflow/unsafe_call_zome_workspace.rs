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

    pub async fn apply_ref<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a CallZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Result<R, WorkspaceError> {
        todo!()
    }

    pub async fn apply_mut<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a mut CallZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Result<R, WorkspaceError> {
        todo!()
    }
}
