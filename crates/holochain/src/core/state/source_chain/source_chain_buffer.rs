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
    prelude::{Readable, Writer},
};
use sx_types::chain_header::HeaderAddress;
use sx_types::entry::EntryAddress;
use sx_types::{
    agent::AgentId,
    chain_header::ChainHeader,
    entry::Entry,
    prelude::*,
    signature::{Provenance, Signature},
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
    pub fn put_entry(&mut self, entry: Entry, agent_id: &AgentId) -> DatabaseResult<()> {
        let header = header_for_entry(&entry, agent_id, self.chain_head().cloned())?;
        self.sequence.put_header((&header).try_into()?);
        self.cas.put((header, entry))?;
        Ok(())
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
    agent_id: &AgentId,
    prev_head: Option<HeaderAddress>,
) -> Result<ChainHeader, SerializedBytesError> {
    let provenances = &[Provenance::new(agent_id.address(), Signature::fake())];
    let timestamp = chrono::Utc::now().timestamp().into();
    trace!("PUT {} {:?}", entry.address(), entry);
    Ok(ChainHeader::new(
        entry.entry_type(),
        EntryAddress::try_from(entry)?,
        provenances,
        prev_head,
        None,
        None,
        timestamp,
    ))
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
                    self.current = header.prev_header();
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
    use sx_state::{prelude::*, test_utils::test_cell_env};
    use sx_types::{
        entry::Entry,
        prelude::*,
        test_utils::{fake_agent_id, fake_dna},
    };

    #[tokio::test]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        let dna = fake_dna("a");
        let agent_id = fake_agent_id("a");

        let dna_entry = Entry::Dna(Box::new(dna));
        let agent_entry = Entry::AgentId(agent_id.clone());

        env.with_reader(|reader| {
            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_none());
            store.put_entry(dna_entry.clone(), &agent_id)?;
            store.put_entry(agent_entry.clone(), &agent_id)?;
            env.with_commit(|writer| store.flush_to_txn(writer))
        })?;

        env.with_reader(|reader| {
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
        })
    }

    #[tokio::test]
    async fn source_chain_buffer_dump_entries_json() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        let dna = fake_dna("a");
        let agent_id = fake_agent_id("a");

        let dna_entry = Entry::Dna(Box::new(dna));
        let agent_entry = Entry::AgentId(agent_id.clone());

        env.with_reader(|reader| {
            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            store.put_entry(dna_entry.clone(), &agent_id)?;
            store.put_entry(agent_entry.clone(), &agent_id)?;
            env.with_commit(|writer| store.flush_to_txn(writer))
        })?;

        env.with_reader(|reader| {
            let store = SourceChainBuf::new(&reader, &dbs)?;
            let json = store.dump_as_json()?;
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            let parsed = parsed.as_array().unwrap().iter().map(|item| {
                let item = item.as_object().unwrap();
                let header = item.get("header").unwrap();
                // println!("{:?}", &header.get("entry_hash").unwrap().to_vec());
                let entry = item.get("entry").unwrap();
                let entry_type = header.get("entry_type").unwrap().as_str().unwrap();
                let entry_address = header.get("entry_address").unwrap().get("Entry").unwrap().as_array().unwrap();
                let entry_data: serde_json::Value = match entry_type {
                    "AgentId" => entry.get("entry").unwrap().as_object().unwrap().get("pub_sign_key").unwrap().clone(),
                    "Dna" => entry.get("entry").unwrap().as_object().unwrap().get("uuid").unwrap().clone(),
                    _ => serde_json::Value::Null,
                };
                serde_json::json!([entry_type, entry_address, entry_data])
            }).collect::<Vec<_>>();

            assert_eq!(
                "[[\"AgentId\",[80,175,172,157,19,188,197,203,244,17,222,5,124,231,9,136,103,95,220,176,53,29,50,213,177,162,170,128,201,34,105,174,246,127,146,111],\"HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa\"],[\"Dna\",[141,156,107,10,121,153,183,44,252,235,130,18,15,60,195,140,245,216,114,34,159,25,20,192,110,168,173,156,245,222,28,181,205,228,163,32],\"a\"]]",
                &serde_json::to_string(&parsed).unwrap(),
            );

            Ok(())
        })
    }

    // async fn header_for_entry() -> SourceChainResult<()> {}
}
