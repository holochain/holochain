use crate::state::{
    chain_cas::{ChainCasBuffer, HeaderCas},
    chain_sequence::ChainSequenceBuffer,
    source_chain::SourceChainError,
};
use core::ops::Deref;
use sx_state::{
    buffer::StoreBuffer,
    db::{self, DbManager},
    env::ReadManager,
    error::WorkspaceResult,
    prelude::{Readable, Reader, Writer},
};
use sx_types::{
    agent::AgentId,
    chain_header::ChainHeader,
    entry::{entry_type::EntryType, Entry},
    prelude::{Address, AddressableContent},
    signature::{Provenance, Signature},
};

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
        Ok(self
            .cas
            .entries()
            .iter_raw()?
            .filter_map(|(_, e)| match e {
                Entry::AgentId(agent_id) => Some(agent_id),
                _ => None,
            })
            .next())
    }

    pub fn try_commit(&self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

impl<'env, R: Readable> StoreBuffer<'env> for SourceChainBuffer<'env, R> {
    type Error = SourceChainError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> Result<(), Self::Error> {
        self.cas.flush_to_txn(writer)?;
        self.sequence.flush_to_txn(writer)?;
        Ok(())
    }
}

fn header_for_entry(entry: &Entry, agent_id: &AgentId, prev_head: Address) -> ChainHeader {
    let provenances = &[Provenance::new(agent_id.address(), Signature::fake())];
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
    use crate::state::source_chain::SourceChainResult;
    use sx_state::{
        db::DbManager,
        env::{create_lmdb_env, ReadManager, WriteManager},
        error::WorkspaceResult,
        prelude::Reader,
        test_utils::test_env,
    };
    use tempdir::TempDir;

    #[test]
    fn asdf() -> SourceChainResult<()> {
        let env = test_env();
        let dbs = env.dbs()?;
        env.with_reader(|reader| {
            let source_chain = SourceChainBuffer::new(&reader, &dbs)?;
            Ok(())
        })
    }
}
