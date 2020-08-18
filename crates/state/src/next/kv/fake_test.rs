use super::{BufferedStore, KvBuf, Op, Scratch};
use crate::{
    env::{ReadManager, WriteManager},
    error::{DatabaseError, DatabaseResult},
    test_utils::test_cell_env,
};
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use rkv::StoreOptions;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::*;

pub struct OldCallZomeWorkspaceRef<'env> {
    pub source_chain: SourceChainRef<'env>,
    pub meta: MetadataBufRef<'env>,
    pub cache_cas: ElementBufRef<'env>,
    pub cache_meta: MetadataBufRef<'env>,
}

impl<'env> WorkspaceRef for OldCallZomeWorkspaceRef<'env> {

    type Scratch = OldCallZomeWorkspaceScratch;

    fn new(reader: &Reader, scratch: &mut Self::Scratch) -> Self {
        Self {
            source_chain: SourceChainRef::new(reader, &mut scratch.source_chain),
            meta: MetadataBufRef::new(reader, &mut scratch.meta),
            cache_cas: ElementBufRef::new(reader, &mut scratch.cache_cas),
            cache_meta: MetadataBufRef::new(reader, &mut scratch.cache_meta),
        }
    }
}

pub struct OldCallZomeWorkspaceScratch {
    pub source_chain: SourceChainScratch,
    pub meta: MetadataBufScratch,
    pub cache_cas: ElementBufScratch,
    pub cache_meta: MetadataBufScratch,
}

pub struct OldCallZomeWorkspaceReader {
    reader: Reader,
    scratch: &mut OldCallZomeWorkspaceScratch,
}

trait WorkspaceRef: Sized {
    type Scratch;
    fn new(reader: &Reader, scratch: &mut Self::Scratch) -> Self;
}

// Generic?
// Needs to convert
// WorkspaceScratch -> WorkspaceReader -> WorkspaceRef
pub struct BufTransaction<S, Ref>
where
Ref: WorkspaceRef,
S: AsMut<Ref>
{
    reader: Reader,
    scratch: &mut S,
}

impl AsMut for OldCallZomeWorkspaceReader {
    fn as_mut(&mut self) ->  OldCallZomeWorkspaceRef<'_> {
        OldCallZomeWorkspaceRef::new(&self.reader, &mut self.scratch)
    }
}

impl AsRef for OldCallZomeWorkspaceReader {
    fn as_ref(&self) ->  OldCallZomeWorkspaceRef<'_> {
        OldCallZomeWorkspaceRef::new(&self.reader, &self.scratch)
    }
}

impl OldCallZomeWorkspaceRef<'env> {
    pub fn new(reader: &'env Reader, scratch: &mut OldCallZomeWorkspaceScratch) -> Self {
        Self {
            source_chain: SourceChainRef::new(reader, &mut scratch.source_chain),
            meta: MetadataBufRef::new(reader, &mut scratch.meta),
            cache_cas: ElementBufRef::new(reader, &mut scratch.cache_cas),
            cache_meta: MetadataBufRef::new(reader, &mut scratch.cache_meta),
        }
    }
}



#[tokio::test(threaded_scheduler)]
async fn test_commit() -> DatabaseResult<()> {
    let arc = test_cell_env();
    let env = arc.guard().await;
    let db1 = env.inner().open_single("kv1", StoreOptions::create())?;
}
