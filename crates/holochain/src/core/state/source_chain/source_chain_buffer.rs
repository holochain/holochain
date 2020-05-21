use crate::core::state::{
    chain_cas::{ChainCasBuf, HeaderCas},
    chain_sequence::ChainSequenceBuf,
    source_chain::{ChainElement, SignedHeaderHashed, SourceChainError, SourceChainResult},
};
use fallible_iterator::FallibleIterator;
use holochain_state::{buffer::BufferedStore, error::DatabaseResult, prelude::*};
use holochain_types::{
    composite_hash::HeaderAddress,
    entry::{Entry, EntryHashed},
    prelude::*,
    Header, HeaderHashed,
};
use tracing::*;

pub struct SourceChainBuf<'env, R: Readable> {
    cas: ChainCasBuf<'env, R>,
    sequence: ChainSequenceBuf<'env, R>,
    keystore: KeystoreSender,
}

impl<'env, R: Readable> SourceChainBuf<'env, R> {
    pub fn new(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(Self {
            cas: ChainCasBuf::primary(reader, dbs, true)?,
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

    /*pub fn get_entry(&self, k: EntryHash) -> DatabaseResult<Option<Entry>> {
        self.cas.get_entry(k)
    }*/

    pub async fn get_element(&self, k: &HeaderAddress) -> SourceChainResult<Option<ChainElement>> {
        debug!("GET {:?}", k);
        self.cas.get_element(k).await
    }

    pub async fn get_header(
        &self,
        k: &HeaderAddress,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        self.cas.get_header(k).await
    }

    pub fn cas(&self) -> &ChainCasBuf<R> {
        &self.cas
    }

    pub async fn put(
        &mut self,
        header: Header,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderAddress> {
        let header = HeaderHashed::with_data(header).await?;
        let header_address = header.as_hash().to_owned();
        let signed_header = SignedHeaderHashed::new(&self.keystore, header).await?;
        let maybe_entry = match maybe_entry {
            None => None,
            Some(entry) => Some(EntryHashed::with_data(entry).await?),
        };

        /*
        FIXME: this needs to happen here.
        if !header.validate_entry(maybe_entry) {
            return Err(SourceChainError(ChainInvalidReason::HeaderAndEntryMismatch));
        }
        */

        self.sequence.put_header(header_address.clone());
        self.cas.put(signed_header, maybe_entry)?;
        Ok(header_address)
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.cas.headers()
    }

    /// Get the AgentPubKey from the entry committed to the chain.
    /// If this returns None, the chain was not initialized.
    pub fn agent_pubkey(&self) -> DatabaseResult<Option<AgentPubKey>> {
        // TODO: rewrite in terms of just getting the correct Header
        Ok(self
            .cas
            .public_entries()
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
    pub async fn dump_as_json(&self) -> Result<String, SourceChainError> {
        #[derive(Serialize, Deserialize)]
        struct JsonChainElement {
            pub signature: Signature,
            pub header_address: HeaderAddress,
            pub header: Header,
            pub entry: Option<Entry>,
        }

        // TODO fix this.  We shouldn't really have nil values but this would
        // show if the database is corrupted and doesn't have an element
        #[derive(Serialize, Deserialize)]
        struct JsonChainDump {
            element: Option<JsonChainElement>,
        }

        let mut iter = self.iter_back();
        let mut out = Vec::new();

        while let Some(h) = iter.next()? {
            let maybe_element = self.get_element(h.header_address()).await?;
            match maybe_element {
                None => out.push(JsonChainDump { element: None }),
                Some(element) => {
                    let (signed, entry) = element.into_inner();
                    let (header, signature) = signed.into_inner();
                    let (header, header_address) = header.into_inner();
                    out.push(JsonChainDump {
                        element: Some(JsonChainElement {
                            signature,
                            header_address,
                            header,
                            entry,
                        }),
                    });
                }
            }
        }

        Ok(serde_json::to_string_pretty(&out)?)
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

/// FallibleIterator returning SignedHeaderHashed instances from chain
/// starting with the head, moving back to the origin (Dna) header.
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

impl<'env, R: Readable> FallibleIterator for SourceChainBackwardIterator<'env, R> {
    type Item = SignedHeaderHashed;
    type Error = SourceChainError;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        match &self.current {
            None => Ok(None),
            Some(top) => {
                let top = top.to_owned();
                // TODO - Using a block_on here due to FallibleIterator.
                //        We should switch `iter_back()` to produce an async Stream.
                let header: Option<SignedHeaderHashed> = tokio_safe_block_on::tokio_safe_block_on(
                    async { self.store.get_header(&top).await },
                    std::time::Duration::from_secs(10),
                )??;
                self.current = match &header {
                    None => None,
                    Some(header) => header.header().prev_header().map(|h| h.clone()),
                };
                Ok(header)
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
        Header, HeaderHashed,
    };

    fn fixtures() -> (
        AgentPubKey,
        HeaderHashed,
        Option<Entry>,
        HeaderHashed,
        Option<Entry>,
    ) {
        let _ = holochain_crypto::crypto_init_sodium();
        let dna = fake_dna_file("a");
        let agent_pubkey = fake_agent_pubkey_1();

        let agent_entry = Entry::Agent(agent_pubkey.clone());

        let (dna_header, agent_header) = tokio_safe_block_on::tokio_safe_block_on(
            async {
                let dna_header = Header::Dna(header::Dna {
                    author: agent_pubkey.clone(),
                    timestamp: Timestamp::now(),
                    header_seq: 0,
                    hash: dna.dna_hash().clone(),
                });
                let dna_header = HeaderHashed::with_data(dna_header).await.unwrap();

                let agent_header = Header::EntryCreate(header::EntryCreate {
                    author: agent_pubkey.clone(),
                    timestamp: Timestamp::now(),
                    header_seq: 0,
                    prev_header: dna_header.as_hash().to_owned().into(),
                    entry_type: header::EntryType::AgentPubKey,
                    entry_hash: agent_pubkey.clone().into(),
                });
                let agent_header = HeaderHashed::with_data(agent_header).await.unwrap();

                (dna_header, agent_header)
            },
            std::time::Duration::from_secs(1),
        )
        .unwrap();

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
            store
                .put(dna_header.as_content().clone(), dna_entry.clone())
                .await?;
            store
                .put(agent_header.as_content().clone(), agent_entry.clone())
                .await?;
            env.with_commit(|writer| store.flush_to_txn(writer))?;
        };

        {
            let reader = env.reader()?;

            let store = SourceChainBuf::new(&reader, &dbs)?;
            assert!(store.chain_head().is_some());

            // get the full element
            let dna_element_fetched = store
                .get_element(dna_header.as_hash())
                .await
                .expect("error retrieving")
                .expect("entry not found");
            let agent_element_fetched = store
                .get_element(agent_header.as_hash())
                .await
                .expect("error retrieving")
                .expect("entry not found");
            assert_eq!(dna_header.as_content(), dna_element_fetched.header());
            assert_eq!(dna_entry, *dna_element_fetched.entry());
            assert_eq!(agent_header.as_content(), agent_element_fetched.header());
            assert_eq!(agent_entry, *agent_element_fetched.entry());

            // check that you can iterate on the chain
            let mut iter = store.iter_back();
            let mut res = Vec::new();

            while let Some(h) = iter.next()? {
                res.push(
                    store
                        .get_element(h.header_address())
                        .await
                        .unwrap()
                        .unwrap()
                        .header()
                        .clone(),
                );
            }
            assert_eq!(
                vec![
                    agent_header.as_content().clone(),
                    dna_header.as_content().clone(),
                ],
                res
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
            store
                .put(dna_header.as_content().clone(), dna_entry)
                .await?;
            store
                .put(agent_header.as_content().clone(), agent_entry)
                .await?;

            env.with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;

            let store = SourceChainBuf::new(&reader, &dbs)?;
            let json = store.dump_as_json().await?;
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

                    /*let _entry_hash = header
                        .get("entry_hash")
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
                    // serde_json::json!([entry_type, entry_hash, entry_data])
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
