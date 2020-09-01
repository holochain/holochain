#![allow(clippy::mutex_atomic)]
use super::*;
use futures::Future;
use std::{
    marker::PhantomData,
    sync::{atomic::AtomicPtr, Arc},
};
use tokio::sync::RwLock;

#[derive(Clone, shrinkwraprs::Shrinkwrap)]
pub struct CallZomeWorkspaceLock(Arc<RwLock<CallZomeWorkspace>>);

#[deprecated = "remove"]
pub struct CallZomeWorkspaceLockGuard<'env> {
    _workspace: Option<Arc<tokio::sync::RwLock<AtomicPtr<std::ffi::c_void>>>>,
    phantom: PhantomData<&'env ()>,
}

impl CallZomeWorkspaceLock {
    pub fn new(workspace: CallZomeWorkspace) -> Self {
        Self(Arc::new(RwLock::new(workspace)))
    }

    #[deprecated = "remove"]
    pub fn from_mut(_: &mut CallZomeWorkspace) -> (CallZomeWorkspaceLockGuard<'_>, Self) {
        todo!("remove")
    }

    #[deprecated = "remove"]
    pub fn null() -> Self {
        todo!()
    }

    #[deprecated = "remove"]
    pub async unsafe fn apply_mut<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a mut CallZomeWorkspace) -> Fut,
    >(
        &self,
        _: F,
    ) -> Result<R, RibosomeError> {
        todo!()
    }
}
