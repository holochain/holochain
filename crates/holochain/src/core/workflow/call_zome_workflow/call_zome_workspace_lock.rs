#![allow(clippy::mutex_atomic)]
use super::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, shrinkwraprs::Shrinkwrap)]
pub struct CallZomeWorkspaceLock(Arc<RwLock<CallZomeWorkspace>>);

impl CallZomeWorkspaceLock {
    pub fn new(workspace: CallZomeWorkspace) -> Self {
        Self(Arc::new(RwLock::new(workspace)))
    }

    /// ABSOLUTE HACK.
    /// At one point we had an unsafe mechanism for erasing the lifetime of an
    /// LMDB reader, which involved using a raw pointer. This function was
    /// implemented such that the pointer was set null. Then, a lot of test
    /// logic was built on that foundation.
    pub unsafe fn null() -> Self {
        let workspace: CallZomeWorkspace =
            std::mem::transmute([0 as u8; std::mem::size_of::<CallZomeWorkspace>()]);
        Self::new(workspace)
    }
}

impl From<CallZomeWorkspace> for CallZomeWorkspaceLock {
    fn from(w: CallZomeWorkspace) -> Self {
        Self::new(w)
    }
}
