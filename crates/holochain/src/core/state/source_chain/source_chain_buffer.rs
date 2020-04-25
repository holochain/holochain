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
use holochain_types::{
    chain_header::{ChainHeader, ChainElement},
    header,
    entry::Entry,
    prelude::*,
    signature::{Provenance, Signature},
    time::Iso8601,
};
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

    /*pub fn get_entry(&self, k: EntryAddress) -> DatabaseResult<Option<Entry>> {
        self.cas.get_entry(k)
    }*/

    pub fn get_element(&self, k: HeaderAddress) -> DatabaseResult<Option<ChainElement>> {
        self.cas.get_entry(k)
    }

    pub fn get_header(&self, k: HeaderAddress) -> DatabaseResult<Option<ChainHeader>> {
        self.cas.get_header(k)
    }

    pub fn cas(&self) -> &ChainCasBuf<R> {
        &self.cas
    }

    pub fn put_element(&mut self, element: Element) -> DatabaseResult<()> {
        let (_, header, _) = element;
        trace!("PUT {} {:?}", entry.entry_hash(), entry);
        self.sequence.put_header((&header).try_into()?);
        self.cas.put(element)?;
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
        struct JsonChainElement {
            pub signature: Signature,
            pub header: ChainHeader,
            pub entry: Option<Entry>,
        }
        #[derive(Serialize, Deserialize)]
        struct JsonChainDump {
            element: JsonChainElement,
        }

        Ok(serde_json::to_string_pretty(
            &self
                .iter_back()
                .map(|h| {
                    let (signature, header, entry) = self.get_element(h.header_address())?;
                    Ok(JsonChainDump {
                        element: JsonChainElement {
                            signature,
                            header,
                            entry,
                        }
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

    fn fixtures() -> (AgentHash, ChainElement,ChainElement) {
        let dna = fake_dna("a");
        let agent_hash = fake_agent_hash("a");

        let dna_entry = Entry::Dna(Box::new(dna));
        let agent_entry = Entry::AgentKey(agent_hash.clone());

        let dna_header = ChainHeader::Dna(
            header::Dna {
                timestamp: chrono::Utc::now().timestamp().into(),
                author: agent_hash.clone(),
                dna.hash(),
            }
        );
        let dna_element = ChainElement(Signature::fake(), dna_header, Some(dna_entry));

        let agent_header = ChainHeader::(
            header::EntryCreate {
                timestamp: chrono::Utc::now().timestamp().into(),
                author: agent_hash.clone(),
                prev_headr: dna_header.hash(),
                entry_type: header::EntryType::AgentKey,
                entry_address: agent_hash.into(),
            }
        );
        let agent_element = ChainElement(Signature::fake(), agent_header, Some(agent_entry));
        (agent_hash, dna_element, agent_element)
    }

    #[tokio::test]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        let (agent_hash, dna_element, agent_element) = fixtures();

        env.with_reader(|reader| {
            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_none());
            store.put_element(dna_element.clone())?;
            store.put_element(agent_element.clone())?;
            env.with_commit(|writer| store.flush_to_txn(writer))
        })?;

        env.with_reader(|reader| {
            let store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_some());

            // get the full element
            let dna_element_fetched = store
                .get_element((&dna_header).hash().into())
                .expect("error retrieving")
                .expect("entry not found");
            let agent_element_fetched = store
                .get_element((&agent_header).hash().into())
                .expect("error retrieving")
                .expect("entry not found");
            assert_eq!(dna_element, dna_element_fetched);
            assert_eq!(agent_element, agent_element_fetched);

            /* get just the entries
            let dna_entry_fetched = store
                .get_entry((&dna_entry).try_into()?)
                .expect("error retrieving")
                .expect("entry not found");
            let agent_entry_fetched = store
                .get_entry((&agent_entry).try_into()?)
                .expect("error retrieving")
                .expect("entry not found");
            assert_eq!(dna_entry, dna_entry_fetched);
            assert_eq!(agent_entry, agent_entry_fetched);*/

            // check that you can iterate on the chain
            assert_eq!(
                store
                    .iter_back()
                    .map(|h| Ok(store.get_element(h.header_address())?))
                    .collect::<Vec<_>>()
                    .unwrap(),
                vec![Some(agent_element), Some(dna_element)]
            );
            Ok(())
        })
    }

    #[tokio::test]
    async fn source_chain_buffer_dump_entries_json() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        let (agent_hash, dna_element, agent_element) = fixtures();

        env.with_reader(|reader| {
            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            store.put_element(dna_element.clone())?;
            store.put_element(agent_element.clone())?;
            env.with_commit(|writer| store.flush_to_txn(writer))
        })?;

        env.with_reader(|reader| {
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

            Ok(())
        })
    }

    // async fn header_for_entry() -> SourceChainResult<()> {}
}
