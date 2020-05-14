#![allow(clippy::mutex_atomic)]
use super::*;
use fixt::prelude::*;
use futures::Future;
use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc,
    },
};
use tracing::*;

// TODO write tests to verify the invariant.
/// This is needed to use the database where
/// the lifetimes cannot be verified by
/// the compiler (e.g. with wasmer).
/// The checks are moved to runtime.
/// The api is non-blocking because this
/// should never be contested if the invariant is held.
/// This type cannot write to the db.
/// It only takes a [Reader].
/// ## Thread Safety
/// This type is not `Send` or `Sync` and cannot be
/// shared between threads.
/// It's best to imagine it like a regular `&` except that it
/// is enforced at runtime.
/// ### Mutex
/// A mutex is used to guarantee that no one else is reading or
/// writing to the data but this is never contested
/// because of the single threaded nature.
/// Default is used to avoid serde
#[derive(Debug, Clone, Default)]
pub struct UnsafeInvokeZomeWorkspace {
    workspace: std::sync::Weak<std::sync::Mutex<AtomicPtr<std::ffi::c_void>>>,
}

fixturator!(
    UnsafeInvokeZomeWorkspace,
    {
        /// Useful when we need this type for tests where we don't want to use it.
        /// It will always return None.
        let fake_ptr = std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();
        let guard = Arc::new(std::sync::Mutex::new(TrustedToBeThreadsafePointer(
            fake_ptr,
        )));
        let workspace = Arc::downgrade(&guard);
        // Make sure the weak Arc cannot be upgraded
        std::mem::drop(guard);
        UnsafeInvokeZomeWorkspace { workspace }
    },
    {
        UnsafeInvokeZomeWorkspaceFixturator::new(Empty)
            .next()
            .unwrap()
    },
    {
        UnsafeInvokeZomeWorkspaceFixturator::new(Empty)
            .next()
            .unwrap()
    }
);

/// if it was safe code we wouldn't need trust
unsafe impl Send for TrustedToBeThreadsafePointer {}

// TODO: SAFETY: Tie the guard to the lmdb `'env` lifetime.
/// If this guard is dropped the underlying
/// ptr cannot be used.
/// ## Safety
/// Don't use `mem::forget` on this type as it will
/// break the checks.
pub struct UnsafeInvokeZomeWorkspaceGuard<'env> {
    workspace: Option<Arc<std::sync::Mutex<AtomicPtr<std::ffi::c_void>>>>,
    phantom: PhantomData<&'env ()>,
}

impl UnsafeInvokeZomeWorkspace {
    pub fn from_mut<'env>(
        workspace: &'env mut InvokeZomeWorkspace,
    ) -> (UnsafeInvokeZomeWorkspaceGuard<'env>, Self) {
        let raw_ptr = workspace as *mut InvokeZomeWorkspace as *mut std::ffi::c_void;
        let guard = Arc::new(std::sync::Mutex::new(AtomicPtr::new(raw_ptr)));
        let workspace = Arc::downgrade(&guard);
        let guard = UnsafeInvokeZomeWorkspaceGuard {
            workspace: Some(guard),
            phantom: PhantomData,
        };
        let workspace = Self { workspace };
        (guard, workspace)
    }

    #[cfg(test)]
    /// Useful when we need this type for tests where we don't want to use it.
    /// It will always return None.
    pub fn test_dropped_guard() -> Self {
        let fake_ptr = std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();
        let guard = Arc::new(std::sync::Mutex::new(AtomicPtr::new(fake_ptr)));
        let workspace = Arc::downgrade(&guard);
        // Make sure the weak Arc cannot be upgraded
        std::mem::drop(guard);
        Self { workspace }
    }

    pub async unsafe fn apply_ref<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a InvokeZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Option<R> {
        // Check it exists
        match self.workspace.upgrade() {
            // Check that no-one else can write
            Some(lock) => match lock
                .try_lock()
                .map_err(|e| {
                    if let std::sync::TryLockError::WouldBlock = e {
                        error!(
                            "{}{}{}",
                            "Failed to get lock on unsafe type. ",
                            "This means the lock is being used by multiple threads.",
                            "This is a BUG and should not be happening"
                        );
                    }
                    e
                })
                .ok()
            {
                Some(guard) => {
                    let sc = guard.load(Ordering::SeqCst);
                    let sc = sc as *const InvokeZomeWorkspace;
                    match sc.as_ref() {
                        Some(s) => Some(f(s).await),
                        None => None,
                    }
                }
                None => None,
            },
            None => None,
        }
    }

    pub async unsafe fn apply_mut<
        'a,
        R,
        Fut: Future<Output = R> + 'a,
        F: FnOnce(&'a mut InvokeZomeWorkspace) -> Fut,
    >(
        &self,
        f: F,
    ) -> Option<R> {
        // Check it exists
        match self.workspace.upgrade() {
            // Check that no-one else can write
            Some(lock) => match lock
                .try_lock()
                .map_err(|e| {
                    if let std::sync::TryLockError::WouldBlock = e {
                        error!(
                            "{}{}{}",
                            "Failed to get lock on unsafe type. ",
                            "This means the lock is being used by multiple threads.",
                            "This is a BUG and should not be happening"
                        );
                    }
                    e
                })
                .ok()
            {
                Some(guard) => {
                    let sc = guard.load(Ordering::SeqCst);
                    let sc = sc as *mut InvokeZomeWorkspace;
                    match sc.as_mut() {
                        Some(s) => Some(f(s).await),
                        None => None,
                    }
                }
                None => None,
            },
            None => None,
        }
    }
}

impl Drop for UnsafeInvokeZomeWorkspaceGuard<'_> {
    fn drop(&mut self) {
        Arc::try_unwrap(self.workspace.take().expect("BUG: This has to be here"))
            .expect("BUG: Invariant broken, strong reference active while guard is dropped");
    }
}
