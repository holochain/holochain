use crate::core::state::{
    chain_cas::{ChainCasBuf, HeaderCas},
    chain_sequence::ChainSequenceBuf,
    source_chain::SourceChainError,
};

use fallible_iterator::FallibleIterator;
use holochain_state::{
    buffer::BufferedStore,
    db::DbManager,
    error::DatabaseResult,
    prelude::{Readable, Writer},
};
use holochain_types::chain_header::HeaderAddress;
use holochain_types::entry::EntryAddress;
use holochain_types::{chain_header::ChainHeader, entry::Entry, prelude::*, time::Iso8601};
use tracing::*;

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

    // add a cache test only method that allows this to
    // be used with the cache database for testing
    // FIXME This should only be cfg(test) but that doesn't work with integration tests
    pub fn cache(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        Ok(Self {
            cas: ChainCasBuf::cache(reader, dbs)?,
            sequence: ChainSequenceBuf::new(reader, dbs)?,
        })
    }

    pub fn chain_head(&self) -> Option<&HeaderAddress> {
        self.sequence.chain_head()
    }

    pub fn get_entry(&self, k: EntryAddress) -> DatabaseResult<Option<Entry>> {
        self.cas.get_entry(k)
    }

    pub fn get_header(&self, k: HeaderAddress) -> DatabaseResult<Option<ChainHeader>> {
        self.cas.get_header(k)
    }

    pub fn cas(&self) -> &ChainCasBuf<R> {
        &self.cas
    }

    // FIXME: put this function in SourceChain, replace with simple put_entry and put_header
    #[allow(dead_code, unreachable_code)]
    pub fn put_entry(&mut self, entry: Entry, agent_hash: &AgentHash) -> DatabaseResult<()> {
        let header = header_for_entry(&entry, agent_hash, self.chain_head().cloned())?;
        self.sequence.put_header((&header).try_into()?);
        self.cas.put((header, entry))?;
        Ok(())
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.cas.headers()
    }

    /// Get the AgentHash from the entry committed to the chain.
    /// If this returns None, the chain was not initialized.
    pub fn agent_hash(&self) -> DatabaseResult<Option<AgentHash>> {
        Ok(self
            .cas
            .entries()
            .iter_raw()?
            .filter_map(|(_, e)| match e {
                Entry::AgentKey(agent_hash) => Some(agent_hash),
                _ => None,
            })
            .next())
    }

    pub fn iter_back(&'env self) -> SourceChainBackwardIterator<'env, R> {
        SourceChainBackwardIterator::new(self)
    }

    /// dump the entire source chain as a pretty-printed json string
    pub fn dump_as_json(&self) -> Result<String, SourceChainError> {
        #[derive(Serialize, Deserialize)]
        struct JsonChainDump {
            pub header: ChainHeader,
            pub entry: Option<Entry>,
        }

        Ok(serde_json::to_string_pretty(
            &self
                .iter_back()
                .map(|h| {
                    Ok(JsonChainDump {
                        entry: self.get_entry(h.entry_address().to_owned())?,
                        header: h,
                    })
                })
                .collect::<Vec<_>>()?,
        )?)
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

fn header_for_entry(
    entry: &Entry,
    agent_hash: &AgentHash,
    prev_head: Option<HeaderAddress>,
) -> Result<ChainHeader, SerializedBytesError> {
    let _provenances = holochain_types::test_utils::fake_provenance_for_agent(&agent_hash);
    let _timestamp: Iso8601 = chrono::Utc::now().timestamp().into();
    trace!("PUT {} {:?}", entry.entry_hash(), entry);
    Ok(ChainHeader {
        entry_address: EntryAddress::try_from(entry)?,
        prev_header_address: prev_head,
    })
}

pub struct SourceChainBackwardIterator<'env, R: Readable> {
    store: &'env SourceChainBuf<'env, R>,
    current: Option<HeaderAddress>,
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
                if let Some(header) = self.store.get_header(top.to_owned())? {
                    self.current = header.prev_header_address().cloned();
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
    use crate::core::state::source_chain::SourceChainResult;
    use fallible_iterator::FallibleIterator;
    use holochain_state::{prelude::*, test_utils::test_cell_env};
    use holochain_types::{
        entry::Entry,
        prelude::*,
        test_utils::{fake_agent_hash, fake_dna},
    };

    #[tokio::test]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        let dna = fake_dna("a");
        let agent_hash = fake_agent_hash("a");

        let dna_entry = Entry::Dna(Box::new(dna));
        let agent_entry = Entry::AgentKey(agent_hash.clone());

        {
            let reader = env.reader()?;

            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_none());
            store.put_entry(dna_entry.clone(), &agent_hash)?;
            store.put_entry(agent_entry.clone(), &agent_hash)?;
            env.with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;

            let store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_some());
            let dna_entry_fetched = store
                .get_entry((&dna_entry).try_into()?)
                .expect("error retrieving")
                .expect("entry not found");
            let agent_entry_fetched = store
                .get_entry((&agent_entry).try_into()?)
                .expect("error retrieving")
                .expect("entry not found");
            assert_eq!(dna_entry, dna_entry_fetched);
            assert_eq!(agent_entry, agent_entry_fetched);
            assert_eq!(
                store
                    .iter_back()
                    .map(|h| Ok(store.get_entry(h.entry_address().to_owned())?))
                    .collect::<Vec<_>>()
                    .unwrap(),
                vec![Some(agent_entry), Some(dna_entry)]
            );
            Ok(())
        }
    }

    #[tokio::test]
    async fn source_chain_buffer_dump_entries_json() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        let dna = fake_dna("a");
        let agent_hash = fake_agent_hash("a");

        let dna_entry = Entry::Dna(Box::new(dna));
        let agent_entry = Entry::AgentKey(agent_hash.clone());

        {
            let reader = env.reader()?;

            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            store.put_entry(dna_entry.clone(), &agent_hash)?;
            store.put_entry(agent_entry.clone(), &agent_hash)?;
            env.with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;

            let store = SourceChainBuf::new(&reader, &dbs)?;
            let json = store.dump_as_json()?;
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            let parsed = parsed
                .as_array()
                .unwrap()
                .iter()
                .map(|item| {
                    let item = item.as_object().unwrap();
                    let header = item.get("header").unwrap();
                    let entry = item.get("entry").unwrap();
                    dbg!(entry);
                    let _entry_address = header
                        .get("entry_address")
                        .unwrap()
                        .get("Entry")
                        .unwrap()
                        .as_array()
                        .unwrap();
                    let entry_type = entry.get("entry_type").unwrap().as_str().unwrap();
                    let _entry_data: serde_json::Value = match entry_type {
                        "AgentKey" => entry.get("entry").unwrap().clone(),
                        "Dna" => entry
                            .get("entry")
                            .unwrap()
                            .as_object()
                            .unwrap()
                            .get("uuid")
                            .unwrap()
                            .clone(),
                        _ => serde_json::Value::Null,
                    };
                    // FIXME: this test is very specific; commenting out the specifics for now
                    // until we finalize the Entry and Header format
                    // serde_json::json!([entry_type, entry_address, entry_data])
                    serde_json::json!(entry_type)
                })
                .collect::<Vec<_>>();

            assert_eq!(
                "[\"AgentKey\",\"Dna\"]",
                &serde_json::to_string(&parsed).unwrap(),
            );
        }
        Ok(())
    }
}
