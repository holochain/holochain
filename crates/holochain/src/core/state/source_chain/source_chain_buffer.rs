use crate::core::state::{
    chain_cas::{ChainCasBuf, HeaderCas},
    chain_sequence::ChainSequenceBuf,
    source_chain::SourceChainError,
};

use sx_state::{
    buffer::BufferedStore,
    db::DbManager,
    error::DatabaseResult,
    prelude::{Readable, Writer},
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
    pub fn put_entry(&mut self, entry: Entry) -> () {
        let _header = header_for_entry(&entry, unimplemented!(), unimplemented!());
        self.cas.put((_header, entry));
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
}

impl<'env, R: Readable> BufferedStore<'env> for SourceChainBuf<'env, R> {
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

    use super::SourceChainBuf;
    use crate::core::state::source_chain::SourceChainResult;
    use sx_state::{env::ReadManager, test_utils::test_env};


    #[tokio::test]
    async fn header_for_entry() -> SourceChainResult<()> {
        // TODO: write test
        let arc = test_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        env.with_reader(|reader| {
            let _source_chain = SourceChainBuf::new(&reader, &dbs)?;
            Ok(())
        })
    }
}
