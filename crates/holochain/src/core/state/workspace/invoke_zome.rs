use super::Workspace;
use crate::core::state::{
    cascade::Cascade, chain_cas::ChainCasBuf, chain_meta::ChainMetaBuf, source_chain::SourceChain,
    workspace::WorkspaceResult,
};
use holochain_state::{db::DbManager, prelude::*};

pub struct InvokeZomeWorkspace<'env> {
    pub source_chain: SourceChain<'env, Reader<'env>>,
    pub meta: ChainMetaBuf<'env, ()>,
    pub cache_cas: ChainCasBuf<'env, Reader<'env>>,
    pub cache_meta: ChainMetaBuf<'env, ()>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbs: &'env DbManager) -> WorkspaceResult<Self> {
        let source_chain = SourceChain::new(reader, dbs)?;

        let cache_cas = ChainCasBuf::cache(reader, dbs)?;
        let meta = ChainMetaBuf::primary(reader, dbs)?;
        let cache_meta = ChainMetaBuf::cache(reader, dbs)?;

        Ok(InvokeZomeWorkspace {
            source_chain,
            meta,
            cache_cas,
            cache_meta,
        })
    }

    pub fn cascade(&self) -> Cascade {
        Cascade::new(
            &self.source_chain.cas(),
            &self.meta,
            &self.cache_cas,
            &self.cache_meta,
        )
    }
}

impl<'env> Workspace for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.into_inner().flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}

pub mod raw {
    #![allow(clippy::mutex_atomic)]
    use super::*;
    use futures::Future;
    use std::{marker::PhantomData, rc::Rc};

    // TODO write tests to varify the invariant.
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
    /// writing to the data the data but this is never contested
    /// because of the single threaded nature.
    pub struct UnsafeInvokeZomeWorkspace {
        workspace: std::rc::Weak<std::sync::Mutex<*mut std::ffi::c_void>>,
    }

    // TODO: SAFETY: Tie the guard to the lmdb `'env` lifetime.
    /// If this guard is dropped the underlying
    /// ptr cannot be used.
    /// ## Safety
    /// Don't use `mem::forget` on this type as it will
    /// break the checks.
    pub struct UnsafeInvokeZomeWorkspaceGuard<'env> {
        workspace: Option<Rc<std::sync::Mutex<*mut std::ffi::c_void>>>,
        phantom: PhantomData<&'env ()>,
    }

    impl UnsafeInvokeZomeWorkspace {
        pub fn from_mut<'env>(
            workspace: &'env mut InvokeZomeWorkspace,
        ) -> (UnsafeInvokeZomeWorkspaceGuard<'env>, Self) {
            let raw_ptr = workspace as *mut InvokeZomeWorkspace as *mut std::ffi::c_void;
            let guard = Rc::new(std::sync::Mutex::new(raw_ptr));
            let workspace = Rc::downgrade(&guard);
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
            let guard = Rc::new(std::sync::Mutex::new(fake_ptr));
            let workspace = Rc::downgrade(&guard);
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
                Some(lock) => match lock.try_lock().ok() {
                    Some(guard) => {
                        let sc = *guard as *const InvokeZomeWorkspace;
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
                Some(lock) => match lock.try_lock().ok() {
                    Some(guard) => {
                        let sc = *guard as *mut InvokeZomeWorkspace;
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
            Rc::try_unwrap(self.workspace.take().expect("BUG: This has to be here"))
                .expect("BUG: Invariant broken, strong reference active while guard is dropped");
        }
    }
}
