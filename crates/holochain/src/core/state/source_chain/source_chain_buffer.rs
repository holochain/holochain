use crate::core::state::{
    chain_cas::{ChainCasBuf, HeaderCas},
    chain_sequence::ChainSequenceBuf,
    source_chain::SourceChainError,
};

use fallible_iterator::FallibleIterator;
use sx_state::{
    buffer::BufferedStore,
    db::DbManager,
    error::DatabaseResult,
    prelude::{Readable, Reader, Writer},
};
use sx_types::{
    agent::AgentId,
    chain_header::ChainHeader,
    entry::Entry,
    prelude::{Address, AddressableContent},
    signature::{Provenance, Signature},
};

pub struct SourceChainBuf<'env, R: Readable> {
    cas: ChainCasBuf<'env, R>,
    sequence: ChainSequenceBuf<'env, R>,
}

impl<'env, R: Readable> SourceChainBuf<'env, R> {
    pub fn new(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        Ok(Self {
            cas: ChainCasBuf::primary(reader, dbs)?,
            sequence: ChainSequenceBuf::new(reader, dbs)?,
        })
    }

    pub fn chain_head(&self) -> Option<&Address> {
        self.sequence.chain_head()
    }

    pub fn get_entry(&self, k: &Address) -> DatabaseResult<Option<Entry>> {
        self.cas.get_entry(k)
    }

    pub fn get_header(&self, k: &Address) -> DatabaseResult<Option<ChainHeader>> {
        self.cas.get_header(k)
    }

    pub fn cas(&self) -> &ChainCasBuf<R> {
        &self.cas
    }

    // FIXME: put this function in SourceChain, replace with simple put_entry and put_header
    #[allow(dead_code, unreachable_code)]
    pub fn put_entry(&mut self, entry: Entry, agent_id: &AgentId) -> () {
        let header = header_for_entry(&entry, agent_id, self.chain_head().cloned());
        self.sequence.put_header(header.address());
        self.cas.put((header, entry));
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.cas.headers()
    }

    /// Get the AgentId from the entry committed to the chain.
    /// If this returns None, the chain was not initialized.
    pub fn agent_id(&self) -> DatabaseResult<Option<AgentId>> {
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

    pub fn iter_back(&'env self) -> SourceChainBackwardIterator<'env, R> {
        SourceChainBackwardIterator::new(self)
    }
}

impl<'env, R: Readable> BufferedStore<'env> for SourceChainBuf<'env, R> {
    type Error = SourceChainError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> Result<(), Self::Error> {
        self.cas.flush_to_txn(writer)?;
        self.sequence.flush_to_txn(writer)?;
        Ok(())
    }
}

fn header_for_entry(entry: &Entry, agent_id: &AgentId, prev_head: Option<Address>) -> ChainHeader {
    let provenances = &[Provenance::new(agent_id.address(), Signature::fake())];
    let timestamp = chrono::Utc::now().timestamp().into();
    let header = ChainHeader::new(
        entry.entry_type(),
        entry.address(),
        provenances,
        prev_head,
        None,
        None,
        timestamp,
    );
    header
}

pub struct SourceChainBackwardIterator<'env, R: Readable> {
    store: &'env SourceChainBuf<'env, R>,
    current: Option<Address>,
}

impl<'env, R: Readable> SourceChainBackwardIterator<'env, R> {
    pub fn new(store: &'env SourceChainBuf<'env, R>) -> Self {
        Self {
            store,
            current: store.chain_head().cloned(),
        }
    }
}

/// Follows ChainHeader.link through every previous Entry (of any EntryType) in the chain
// #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CORE)]
impl<'env, R: Readable> FallibleIterator for SourceChainBackwardIterator<'env, R> {
    type Item = ChainHeader;
    type Error = SourceChainError;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        match &self.current {
            None => Ok(None),
            Some(top) => {
                if let Some(header) = self.store.get_header(top)? {
                    self.current = header.link();
                    Ok(Some(header))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::SourceChainBuf;
    use crate::core::{
        state::source_chain::SourceChainResult,
        test_utils::{fake_agent_id, fake_dna},
    };
    use fallible_iterator::FallibleIterator;
    use sx_state::{prelude::*, test_utils::test_env};
    use sx_types::{entry::Entry, prelude::*};

    #[tokio::test]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        let arc = test_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        let dna = fake_dna("a");
        let agent_id = fake_agent_id("a");

        let dna_entry = Entry::Dna(Box::new(dna));
        let agent_entry = Entry::AgentId(agent_id.clone());

        env.with_reader(|reader| {
            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_none());
            store.put_entry(dna_entry.clone(), &agent_id);
            store.put_entry(agent_entry.clone(), &agent_id);
            env.with_commit(|writer| store.flush_to_txn(writer))
        })?;

        env.with_reader(|reader| {
            let store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_some());
            store.get_entry(&dna_entry.address()).unwrap();
            store.get_entry(&agent_entry.address()).unwrap();
            assert_eq!(
                store
                    .iter_back()
                    .map(|h| Ok(store.get_entry(h.entry_address())?))
                    .collect::<Vec<_>>()
                    .unwrap(),
                vec![Some(dna_entry), Some(agent_entry)]
            );
            Ok(())
        })
    }

    // async fn header_for_entry() -> SourceChainResult<()> {}
}
