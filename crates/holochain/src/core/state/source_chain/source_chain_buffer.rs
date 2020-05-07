use crate::core::state::{
    chain_cas::{ChainCasBuf, HeaderCas},
    chain_sequence::ChainSequenceBuf,
    source_chain::{ChainElement, SignedHeader, SourceChainError, SourceChainResult},
};
use fallible_iterator::FallibleIterator;
use holochain_state::{buffer::BufferedStore, error::DatabaseResult, prelude::*};
use holochain_types::{address::HeaderAddress, entry::Entry, prelude::*, Header};
use tracing::*;

pub struct SourceChainBuf<'env, R: Readable> {
    cas: ChainCasBuf<'env, R>,
    sequence: ChainSequenceBuf<'env, R>,
    keystore: KeystoreSender,
}

impl<'env, R: Readable> SourceChainBuf<'env, R> {
    pub fn new(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(Self {
            cas: ChainCasBuf::primary(reader, dbs)?,
            sequence: ChainSequenceBuf::new(reader, dbs)?,
            keystore: dbs.keystore(),
        })
    }

    // add a cache test only method that allows this to
    // be used with the cache database for testing
    // FIXME This should only be cfg(test) but that doesn't work with integration tests
    pub fn cache(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(Self {
            cas: ChainCasBuf::cache(reader, dbs)?,
            sequence: ChainSequenceBuf::new(reader, dbs)?,
            keystore: dbs.keystore(),
        })
    }

    pub fn chain_head(&self) -> Option<&HeaderAddress> {
        self.sequence.chain_head()
    }

    pub fn len(&self) -> usize {
        self.sequence.len()
    }

    /*pub fn get_entry(&self, k: EntryAddress) -> DatabaseResult<Option<Entry>> {
        self.cas.get_entry(k)
    }*/

    pub fn get_element(&self, k: &HeaderAddress) -> SourceChainResult<Option<ChainElement>> {
        debug!("GET {:?}", k);
        self.cas.get_element(k)
    }

    pub fn get_header(&self, k: &HeaderAddress) -> DatabaseResult<Option<SignedHeader>> {
        self.cas.get_header(k)
    }

    pub fn cas(&self) -> &ChainCasBuf<R> {
        &self.cas
    }

    pub async fn put(
        &mut self,
        header: Header,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<()> {
        let signed_header = SignedHeader::new(&self.keystore, header.to_owned()).await?;

        /*
        FIXME: this needs to happen here.
        if !header.validate_entry(maybe_entry) {
            return Err(SourceChainError(ChainInvalidReason::HeaderAndEntryMismatch));
        }
        */

        self.sequence.put_header(header.hash().into());
        self.cas.put(signed_header, maybe_entry)?;
        Ok(())
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.cas.headers()
    }

    /// Get the AgentPubKey from the entry committed to the chain.
    /// If this returns None, the chain was not initialized.
    pub fn agent_pubkey(&self) -> DatabaseResult<Option<AgentPubKey>> {
        Ok(self
            .cas
            .entries()
            .iter_raw()?
            .filter_map(|(_, e)| match e {
                Entry::Agent(agent_pubkey) => Some(agent_pubkey),
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
            pub header: Header,
            pub entry: Option<Entry>,
        }

        // TODO fix this.  We shouldn't really have nil values but this would
        // show if the database is corrupted and doesn't have an element
        #[derive(Serialize, Deserialize)]
        struct JsonChainDump {
            element: Option<JsonChainElement>,
        }

        Ok(serde_json::to_string_pretty(
            &self
                .iter_back()
                .map(|h| {
                    let maybe_element = self.get_element(&h.header().hash().into())?;
                    match maybe_element {
                        None => Ok(JsonChainDump { element: None }),
                        Some(element) => Ok(JsonChainDump {
                            element: Some(JsonChainElement {
                                signature: element.signature().to_owned(),
                                header: element.header().to_owned(),
                                entry: element.entry().to_owned(),
                            }),
                        }),
                    }
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

/// Follows Header.link through every previous Entry (of any EntryType) in the chain
// #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CORE)]
impl<'env, R: Readable> FallibleIterator for SourceChainBackwardIterator<'env, R> {
    type Item = SignedHeader;
    type Error = SourceChainError;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        match &self.current {
            None => Ok(None),
            Some(top) => {
                if let Some(signed_header) = self.store.get_header(top)? {
                    self.current = signed_header.header().prev_header().map(|h| h.to_owned());
                    Ok(Some(signed_header))
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
        header,
        prelude::*,
        test_utils::{fake_agent_pubkey_1, fake_dna_file},
        Header,
    };

    fn fixtures() -> (AgentPubKey, Header, Option<Entry>, Header, Option<Entry>) {
        let _ = holochain_crypto::crypto_init_sodium();
        let dna = fake_dna_file("a");
        let agent_pubkey = fake_agent_pubkey_1();

        let agent_entry = Entry::Agent(agent_pubkey.clone());

        let dna_header = Header::Dna(header::Dna {
            timestamp: Timestamp::now(),
            author: agent_pubkey.clone(),
            hash: dna.dna_hash().clone(),
        });

        let agent_header = Header::EntryCreate(header::EntryCreate {
            timestamp: Timestamp::now(),
            author: agent_pubkey.clone(),
            prev_header: dna_header.hash().into(),
            entry_type: header::EntryType::AgentPubKey,
            entry_address: agent_pubkey.clone().into(),
        });

        (
            agent_pubkey,
            dna_header,
            None,
            agent_header,
            Some(agent_entry),
        )
    }

    #[tokio::test(threaded_scheduler)]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await;

        let (_agent_pubkey, dna_header, dna_entry, agent_header, agent_entry) = fixtures();

        {
            let reader = env.reader()?;

            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_none());
            store.put(dna_header.clone(), dna_entry.clone()).await?;
            store.put(agent_header.clone(), agent_entry.clone()).await?;
            env.with_commit(|writer| store.flush_to_txn(writer))?;
        };

        {
            let reader = env.reader()?;

            let store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_some());

            // get the full element
            let dna_element_fetched = store
                .get_element(&dna_header.hash().into())
                .expect("error retrieving")
                .expect("entry not found");
            let agent_element_fetched = store
                .get_element(&agent_header.hash().into())
                .expect("error retrieving")
                .expect("entry not found");
            assert_eq!(dna_header, *dna_element_fetched.header());
            assert_eq!(dna_entry, *dna_element_fetched.entry());
            assert_eq!(agent_header, *agent_element_fetched.header());
            assert_eq!(agent_entry, *agent_element_fetched.entry());

            // check that you can iterate on the chain
            assert_eq!(
                store
                    .iter_back()
                    .map(|h| Ok(store
                        .get_element(&h.header().hash().into())?
                        .unwrap()
                        .header()
                        .clone()))
                    .collect::<Vec<_>>()
                    .unwrap(),
                vec![agent_header, dna_header]
            );
        }

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn source_chain_buffer_dump_entries_json() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await;

        let (_agent_pubkey, dna_header, dna_entry, agent_header, agent_entry) = fixtures();

        {
            let reader = env.reader()?;

            let mut store = SourceChainBuf::new(&reader, &dbs)?;
            store.put(dna_header.clone(), dna_entry).await?;
            store.put(agent_header.clone(), agent_entry).await?;
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
                    let element = item.get("element").unwrap();
                    let header = element.get("header").unwrap();
                    let header_type = header.get("type").unwrap().as_str().unwrap();

                    /*let _entry_address = header
                        .get("entry_address")
                        .unwrap()
                        .get("Entry")
                        .unwrap()
                        .as_array()
                        .unwrap();
                    let entry_type = entry.get("entry_type").unwrap().as_str().unwrap();
                    let _entry_data: serde_json::Value = match entry_type {
                        "AgentPubKey" => entry.get("entry").unwrap().clone(),
                        "Dna" => entry
                            .get("entry")
                            .unwrap()
                            .as_object()
                            .unwrap()
                            .get("uuid")
                            .unwrap()
                            .clone(),
                        _ => serde_json::Value::Null,
                    };*/
                    // FIXME: this test is very specific; commenting out the specifics for now
                    // until we finalize the Entry and Header format
                    // serde_json::json!([entry_type, entry_address, entry_data])
                    serde_json::json!(header_type)
                })
                .collect::<Vec<_>>();

            assert_eq!(
                "[\"EntryCreate\",\"Dna\"]",
                &serde_json::to_string(&parsed).unwrap(),
            );
        }

        Ok(())
    }
}
