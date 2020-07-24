use super::ChainInvalidReason;
use crate::core::state::{
    chain_cas::{ElementBuf, HeaderCas},
    chain_sequence::ChainSequenceBuf,
    source_chain::{SourceChainError, SourceChainResult},
};
use fallible_iterator::FallibleIterator;
use holochain_state::db::GetDb;
use holochain_state::{
    buffer::BufferedStore,
    error::DatabaseResult,
    prelude::{Reader, Writer},
};
use holochain_types::{
    dht_op::{ops_from_element, DhtOp},
    element::{ChainElement, SignedHeaderHashed},
    entry::EntryHashed,
    prelude::*,
    HeaderHashed,
};
use holochain_zome_types::{header, Entry, Header};
use tracing::*;

pub struct SourceChainBuf<'env> {
    cas: ElementBuf<'env>,
    sequence: ChainSequenceBuf<'env>,
    keystore: KeystoreSender,
}

impl<'env> SourceChainBuf<'env> {
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(Self {
            cas: ElementBuf::vault(reader, dbs, true)?,
            sequence: ChainSequenceBuf::new(reader, dbs)?,
            keystore: dbs.keystore(),
        })
    }

    // add a cache test only method that allows this to
    // be used with the cache database for testing
    // FIXME This should only be cfg(test) but that doesn't work with integration tests
    pub fn cache(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(Self {
            cas: ElementBuf::cache(reader, dbs)?,
            sequence: ChainSequenceBuf::new(reader, dbs)?,
            keystore: dbs.keystore(),
        })
    }

    pub fn chain_head(&self) -> Option<&HeaderHash> {
        self.sequence.chain_head()
    }

    pub fn len(&self) -> usize {
        self.sequence.len()
    }

    // TODO: TK-01747: Make this check more robust maybe?
    // PERF: This call must be fast
    pub fn has_genesis(&self) -> bool {
        self.sequence.len() >= 3
    }

    pub async fn get_at_index(&self, i: u32) -> SourceChainResult<Option<ChainElement>> {
        if let Some(address) = self.sequence.get(i)? {
            self.get_element(&address).await
        } else {
            Ok(None)
        }
    }

    pub async fn get_element(&self, k: &HeaderHash) -> SourceChainResult<Option<ChainElement>> {
        debug!("GET {:?}", k);
        self.cas.get_element(k).await
    }

    pub async fn get_header(&self, k: &HeaderHash) -> DatabaseResult<Option<SignedHeaderHashed>> {
        self.cas.get_header(k).await
    }

    pub async fn get_incomplete_dht_ops(&self) -> SourceChainResult<Vec<(u32, Vec<DhtOp>)>> {
        let mut ops = Vec::new();
        // FIXME: This collect shouldn't need to happen but the iterator to the db is not Send
        let ops_headers = self
            .sequence
            .get_items_with_incomplete_dht_ops()?
            .collect::<Vec<_>>();
        for (i, header) in ops_headers {
            let op = ops_from_element(
                &self
                    .get_element(&header)
                    .await?
                    .expect("BUG: element in sequence but not cas"),
            )?;
            ops.push((i, op));
        }
        Ok(ops)
    }

    pub fn complete_dht_op(&mut self, i: u32) -> SourceChainResult<()> {
        self.sequence.complete_dht_op(i)
    }

    pub fn cas<'a>(&'a self) -> &'a ElementBuf<'env> {
        &self.cas
    }

    pub fn sequence(&self) -> &ChainSequenceBuf {
        &self.sequence
    }

    /// Add a ChainElement to the source chain, using a fully-formed Header
    pub async fn put_raw(
        &mut self,
        header: Header,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderHash> {
        let header = HeaderHashed::from_content(header).await;
        let header_address = header.as_hash().to_owned();
        let signed_header = SignedHeaderHashed::new(&self.keystore, header).await?;
        let maybe_entry = match maybe_entry {
            None => None,
            Some(entry) => Some(EntryHashed::from_content(entry).await),
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

    pub fn headers(&self) -> &HeaderCas<'env> {
        &self.cas.headers()
    }

    // TODO: TK-01747: Make this check more robust maybe?
    // PERF: This call must be fast
    pub fn has_initialized(&self) -> bool {
        self.len() > 3
    }

    /// Get the AgentPubKey from the entry committed to the chain.
    /// If this returns None, the chain was not initialized.
    pub async fn agent_pubkey(&self) -> SourceChainResult<Option<AgentPubKey>> {
        if let Some(element) = self.get_at_index(2).await? {
            match element.entry().as_option().ok_or_else(|| {
                SourceChainError::InvalidStructure(ChainInvalidReason::GenesisDataMissing)
            })? {
                Entry::Agent(agent_pubkey) => Ok(Some(agent_pubkey.clone())),
                _ => Err(SourceChainError::InvalidStructure(
                    ChainInvalidReason::MalformedGenesisData,
                )),
            }
        } else {
            Ok(None)
        }
    }

    pub fn iter_back(&'env self) -> SourceChainBackwardIterator<'env> {
        SourceChainBackwardIterator::new(self)
    }

    /// dump the entire source chain as a pretty-printed json string
    pub async fn dump_as_json(&self) -> Result<String, SourceChainError> {
        #[derive(Serialize, Deserialize)]
        struct JsonChainElement {
            pub signature: Signature,
            pub header_address: HeaderHash,
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
                    let (header, signature) = signed.into_header_and_signature();
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

    /// Commit the genesis entries to this source chain, making the chain ready
    /// to use as a `SourceChain`
    pub async fn genesis(
        &mut self,
        dna_hash: DnaHash,
        agent_pubkey: AgentPubKey,
        membrane_proof: Option<SerializedBytes>,
    ) -> SourceChainResult<()> {
        // create a DNA chain element and add it directly to the store
        let dna_header = Header::Dna(header::Dna {
            author: agent_pubkey.clone(),
            timestamp: Timestamp::now().into(),
            header_seq: 0,
            hash: dna_hash,
        });
        let dna_header_address = self.put_raw(dna_header, None).await?;

        // create the agent validation entry and add it directly to the store
        let agent_validation_header = Header::AgentValidationPkg(header::AgentValidationPkg {
            author: agent_pubkey.clone(),
            timestamp: Timestamp::now().into(),
            header_seq: 1,
            prev_header: dna_header_address,
            membrane_proof,
        });
        let avh_addr = self.put_raw(agent_validation_header, None).await?;

        // create a agent chain element and add it directly to the store
        let agent_header = Header::EntryCreate(header::EntryCreate {
            author: agent_pubkey.clone(),
            timestamp: Timestamp::now().into(),
            header_seq: 2,
            prev_header: avh_addr,
            entry_type: header::EntryType::AgentPubKey,
            entry_hash: agent_pubkey.clone().into(),
        });
        self.put_raw(agent_header, Some(Entry::Agent(agent_pubkey)))
            .await?;

        Ok(())
    }
}

impl<'env> BufferedStore<'env> for SourceChainBuf<'env> {
    type Error = SourceChainError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> Result<(), Self::Error> {
        self.cas.flush_to_txn(writer)?;
        self.sequence.flush_to_txn(writer)?;
        Ok(())
    }
}

/// FallibleIterator returning SignedHeaderHashed instances from chain
/// starting with the head, moving back to the origin (Dna) header.
pub struct SourceChainBackwardIterator<'env> {
    store: &'env SourceChainBuf<'env>,
    current: Option<HeaderHash>,
}

impl<'env> SourceChainBackwardIterator<'env> {
    pub fn new(store: &'env SourceChainBuf<'env>) -> Self {
        Self {
            store,
            current: store.chain_head().cloned(),
        }
    }
}

impl<'env> FallibleIterator for SourceChainBackwardIterator<'env> {
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
        prelude::*,
        test_utils::{fake_agent_pubkey_1, fake_dna_file},
        HeaderHashed,
    };
    use holochain_zome_types::{header, Entry, Header};

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

        let agent_entry = Entry::Agent(agent_pubkey.clone().into());

        let (dna_header, agent_header) = tokio_safe_block_on::tokio_safe_block_on(
            async {
                let dna_header = Header::Dna(header::Dna {
                    author: agent_pubkey.clone(),
                    timestamp: Timestamp(0, 0).into(),
                    header_seq: 0,
                    hash: dna.dna_hash().clone(),
                });
                let dna_header = HeaderHashed::from_content(dna_header).await;

                let agent_header = Header::EntryCreate(header::EntryCreate {
                    author: agent_pubkey.clone(),
                    timestamp: Timestamp(1, 0).into(),
                    header_seq: 1,
                    prev_header: dna_header.as_hash().to_owned().into(),
                    entry_type: header::EntryType::AgentPubKey,
                    entry_hash: agent_pubkey.clone().into(),
                });
                let agent_header = HeaderHashed::from_content(agent_header).await;

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
                .put_raw(dna_header.as_content().clone(), dna_entry.clone())
                .await?;
            store
                .put_raw(agent_header.as_content().clone(), agent_entry.clone())
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
            assert_eq!(dna_entry.as_ref(), dna_element_fetched.entry().as_option());
            assert_eq!(agent_header.as_content(), agent_element_fetched.header());
            assert_eq!(
                agent_entry.as_ref(),
                agent_element_fetched.entry().as_option()
            );

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

        let (_agent_pubkey, dna_header, dna_entry, agent_header, agent_entry) = fixtures();

        {
            let reader = env.reader()?;

            let mut store = SourceChainBuf::new(&reader, &env)?;
            store
                .put_raw(dna_header.as_content().clone(), dna_entry)
                .await?;
            store
                .put_raw(agent_header.as_content().clone(), agent_entry)
                .await?;

            env.with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;

            let store = SourceChainBuf::new(&reader, &env)?;
            let json = store.dump_as_json().await?;
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed[0]["element"]["header"]["type"], "EntryCreate");
            assert_eq!(parsed[0]["element"]["header"]["entry_type"], "AgentPubKey");
            assert_eq!(parsed[0]["element"]["entry"]["entry_type"], "Agent");
            assert_ne!(
                parsed[0]["element"]["entry"]["entry"],
                serde_json::Value::Null
            );

            assert_eq!(parsed[1]["element"]["header"]["type"], "Dna");
            assert_eq!(parsed[1]["element"]["entry"], serde_json::Value::Null);
        }

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_header_cas_roundtrip() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let reader = env.reader().unwrap();
        let mut store = SourceChainBuf::new(&reader, &env).unwrap();

        let (_, hashed, _, _, _) = fixtures();
        let header = hashed.into_content();
        let hash = HeaderHash::with_data(&header).await;
        let hashed = HeaderHashed::from_content(header.clone()).await;
        assert_eq!(hash, *hashed.as_hash());

        store.put_raw(header, None).await.unwrap();
        let signed_header = store.get_header(&hash).await.unwrap().unwrap();

        assert_eq!(signed_header.as_hash(), hashed.as_hash());
        assert_eq!(signed_header.as_hash(), signed_header.header_address());
    }
}
