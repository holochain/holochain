

use crate::error::WorkspaceResult;

pub trait Workspace: Sized {
    fn finalize(self) -> WorkspaceResult<()>;
}

pub trait Store: Sized {
    fn finalize(self) -> WorkspaceResult<()>;
}

/// A light wrapper around an arbitrary key-value store
struct KvDb;

///
struct KvStore;
impl Store for KvStore {
    fn finalize(self) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

struct TabularStore;
impl Store for TabularStore {
    fn finalize(self) -> WorkspaceResult<()> {
        unimplemented!()
    }
}


pub struct InvokeZomeWorkspace {
    cas: KvStore,
    meta: TabularStore,
}

/// There can be a different set of db cursors (all writes) that only get accessed in the finalize stage,
/// but other read-only cursors during the actual workflow
pub struct AppValidationWorkspace;

impl Workspace for InvokeZomeWorkspace {
    fn finalize(self) -> WorkspaceResult<()> {
        self.cas.finalize()?;
        self.meta.finalize()?;
        Ok(())
    }
}

impl Workspace for AppValidationWorkspace {
    fn finalize(self) -> WorkspaceResult<()> {
        Ok(())
    }
}
