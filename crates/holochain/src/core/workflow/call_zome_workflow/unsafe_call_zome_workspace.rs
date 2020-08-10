#![allow(clippy::mutex_atomic)]
use super::*;
use futures::Future;
use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc,
    },
};

// TODO write tests to verify the invariant.
/// This is needed to use the database where
/// the lifetimes cannot be verified by
/// the compiler (e.g. with wasmer).
/// The checks are moved to runtime.
/// This type cannot write to the db.
/// It only takes a [Reader].
/// ## Thread Safety
/// This type is `Send` and `Sync`
/// It's best to imagine it like a regular `&` except that it
/// is enforced at runtime.
/// ### Mutex
/// A mutex is used to guarantee that no one else is reading or
/// writing to the data but this is never contested
/// because of the single threaded nature.
/// Default is used to avoid serde
#[derive(Debug, Clone, Default)]
pub struct UnsafeCallZomeWorkspace {
    workspace: std::sync::Weak<tokio::sync::RwLock<AtomicPtr<std::ffi::c_void>>>,
}

// TODO: SAFETY: Tie the guard to the lmdb `'env` lifetime.
/// If this guard is dropped the underlying
/// ptr cannot be used.
/// ## Safety
/// Don't use `mem::forget` on this type as it will
/// break the checks.
pub struct UnsafeCallZomeWorkspaceGuard<'env> {
    workspace: Option<Arc<tokio::sync::RwLock<AtomicPtr<std::ffi::c_void>>>>,
    phantom: PhantomData<&'env ()>,
}

impl UnsafeCallZomeWorkspace {
    pub fn from_mut<'env>(
        workspace: &'env mut CallZomeWorkspace,
    ) -> (UnsafeCallZomeWorkspaceGuard<'env>, Self) {
        let raw_ptr = workspace as *mut CallZomeWorkspace as *mut std::ffi::c_void;
        let guard = Arc::new(tokio::sync::RwLock::new(AtomicPtr::new(raw_ptr)));
        let workspace = Arc::downgrade(&guard);
        let guard = UnsafeCallZomeWorkspaceGuard {
            workspace: Some(guard),
            phantom: PhantomData,
        };
        let workspace = Self { workspace };
        (guard, workspace)
    }

    /// Useful when we need this type where we don't want to use it.
    /// It will always return None.
    pub fn null() -> Self {
        let fake_ptr = std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();
        let guard = Arc::new(tokio::sync::RwLock::new(AtomicPtr::new(fake_ptr)));
        let workspace = Arc::downgrade(&guard);
        // Make sure the weak Arc cannot be upgraded
        std::mem::drop(guard);
        Self { workspace }
    }

    pub async unsafe fn apply_ref<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a CallZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Result<R, error::UnsafeCallZomeWorkspaceError> {
        // Check it exists
        match self.workspace.upgrade() {
            // Check that no-one else can write
            Some(lock) => {
                let guard = lock.read().await;
                let s = {
                    let sc = guard.load(Ordering::SeqCst);
                    let sc = sc as *const CallZomeWorkspace;
                    match sc.as_ref() {
                        Some(s) => s,
                        None => Err(error::UnsafeCallZomeWorkspaceError::GuardDropped)?,
                    }
                };
                Ok(f(s).await)
            }
            None => Err(error::UnsafeCallZomeWorkspaceError::GuardDropped),
        }
    }

    pub async unsafe fn apply_mut<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a mut CallZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Result<R, error::UnsafeCallZomeWorkspaceError> {
        // Check it exists
        match self.workspace.upgrade() {
            // Check that no-one else can write
            Some(lock) => {
                let guard = lock.write().await;
                let s = {
                    let sc = guard.load(Ordering::SeqCst);
                    let sc = sc as *mut CallZomeWorkspace;
                    match sc.as_mut() {
                        Some(s) => s,
                        None => Err(error::UnsafeCallZomeWorkspaceError::GuardDropped)?,
                    }
                };
                Ok(f(s).await)
            }
            None => Err(error::UnsafeCallZomeWorkspaceError::GuardDropped),
        }
    }
}

impl Drop for UnsafeCallZomeWorkspaceGuard<'_> {
    fn drop(&mut self) {
        if let Err(arc) = Arc::try_unwrap(self.workspace.take().expect("BUG: This has to be here"))
        {
            warn!(
                "Trying to drop UnsafeCallZomeWorkspace but there must be outstanding references"
            );
            // Wait on the lock to check if others have it
            tokio_safe_block_on::tokio_safe_block_on(
                arc.write(),
                std::time::Duration::from_secs(10),
            )
            .ok();
            // TODO: B-01648: Try to consume now hoping noone has taken a lock in the meantime
            Arc::try_unwrap(arc).expect(
                "UnsafeCallZomeWorkspace still has live references when workflow is finished",
            );
        }
    }
}

pub mod error {
    use thiserror::Error;
    #[derive(Error, Debug)]
    pub enum UnsafeCallZomeWorkspaceError {
        #[error(
            "The guard for this workspace has been dropped and this workspace is no loanger valid"
        )]
        GuardDropped,
    }
}
