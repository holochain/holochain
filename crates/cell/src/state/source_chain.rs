use super::{
    chain_cas::{ChainCasBuffer, HeaderCas},
    chain_sequence::ChainSequenceBuffer,
};
use core::ops::Deref;
use sx_state::{
    buffer::StoreBuffer,
    db::{self, DbManager},
    env::ReadManager,
    error::WorkspaceResult,
    Readable, Reader, Writer,
};
use sx_types::{
    chain_header::ChainHeader,
    entry::{entry_type::EntryType, Entry},
    prelude::{Address, AddressableContent},
    signature::{Provenance, Signature}, agent::AgentId,
};
use crate::agent::error::SourceChainError;

pub struct SourceChainBuffer<'env, R: Readable> {
    cas: ChainCasBuffer<'env, R>,
    sequence: ChainSequenceBuffer<'env, R>,
}

impl<'env, R: Readable> SourceChainBuffer<'env, R> {
    pub fn new(reader: &'env R, dbs: &'env DbManager) -> WorkspaceResult<Self> {
        Ok(Self {
            cas: ChainCasBuffer::primary(reader, dbs)?,
            sequence: ChainSequenceBuffer::new(reader, dbs)?,
        })
    }

    fn initialize() -> WorkspaceResult<()> {
        unimplemented!()
    }

    pub fn chain_head(&self) -> Option<&Address> {
        self.sequence.chain_head()
    }

    pub fn get_entry(&self, k: &Address) -> WorkspaceResult<Option<Entry>> {
        self.cas.get_entry(k)
    }

    pub fn get_header(&self, k: &Address) -> WorkspaceResult<Option<ChainHeader>> {
        self.cas.get_header(k)
    }

    pub fn cas(&self) -> &ChainCasBuffer<R> {
        &self.cas
    }

    pub fn put_entry(&mut self, entry: Entry) -> () {
        let header = header_for_entry(&entry, unimplemented!(), unimplemented!());
        self.cas.put((header, entry));
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.cas.headers()
    }

    /// Get the AgentId from the entry committed to the chain.
    /// If this returns None, the chain was not initialized.
    pub fn agent_id(&self) -> WorkspaceResult<Option<AgentId>> {
        Ok(self.cas.entries().iter_raw()?.filter_map(|(_, e)| match e {
            Entry::AgentId(agent_id) => Some(agent_id),
            _ => None
        }).next())
    }

    pub fn try_commit(&self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

impl<'env, R: Readable> StoreBuffer<'env> for SourceChainBuffer<'env, R> {
    type Error = SourceChainError;

    fn finalize(self, writer: &'env mut Writer) -> Result<(), Self::Error> {
        self.cas.finalize(writer)?;
        self.sequence.finalize(writer)?;
        Ok(())
    }
}


fn header_for_entry(entry: &Entry, agent_id: &AgentId, prev_head: Address) -> ChainHeader {
    let provenances = &[Provenance::new(
        agent_id.address(),
        Signature::fake(),
    )];
    let timestamp = chrono::Utc::now().timestamp().into();
    let header = ChainHeader::new(
        entry.entry_type(),
        entry.address(),
        provenances,
        Some(prev_head),
        None,
        None,
        timestamp,
    );
    header
}

#[cfg(test)]
pub mod tests {

    use super::{SourceChainBuffer, StoreBuffer};
    use sx_state::{
        env::{create_lmdb_env, DbManager, ReadManager, WriteManager},
        error::WorkspaceResult,
        test_utils::test_env,
        Reader,
    };
    use tempdir::TempDir;
    use crate::agent::error::SourceChainResult;

    #[test]
    fn asdf() -> SourceChainResult<()> {
        let arc = test_env();
        let env = arc.env();
        let dbs = arc.dbs()?;
        arc.env().with_reader(|reader| {
            let source_chain = SourceChainBuffer::new(&reader, &dbs)?;
            Ok(())
        })
    }
}
