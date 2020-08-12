#![allow(clippy::mutex_atomic)]
use super::*;
use futures::Future;
use holochain_state::env::{EnvironmentRead, EnvironmentWrite};
use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc,
    },
};

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
    /// Useful when we need this type where we don't want to use it.
    /// It will always return None.
    pub fn null() -> Self {
        todo!()
    }

    pub async unsafe fn apply_ref<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a CallZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Result<R, error::CallZomeWorkspaceFactoryError> {
        todo!()
    }

    pub async unsafe fn apply_mut<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a mut CallZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Result<R, error::CallZomeWorkspaceFactoryError> {
        todo!()
    }
}

pub mod error {
    use thiserror::Error;
    #[derive(Error, Debug)]
    pub enum CallZomeWorkspaceFactoryError {
        #[error(
            "The guard for this workspace has been dropped and this workspace is no loanger valid"
        )]
        GuardDropped,
    }
}
