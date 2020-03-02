use sx_state::db::CHAIN_HEADERS;
use sx_state::db::CHAIN_ENTRIES;
use crate::agent::error::{SourceChainError, SourceChainResult, ChainInvalidReason};
use serde::{de::DeserializeOwned, Serialize};
use sx_state::{
    buffer::{CasBuffer, StoreBuffer},
    error::WorkspaceResult,
    RkvEnv, Writer, db::DbManager, Reader, SingleStore,
};
use sx_types::{
    chain_header::{HeaderWithEntry, ChainHeader},
    entry::Entry,
    prelude::{Address, AddressableContent},
};

pub type EntryCas<'env> = CasBuffer<'env, Entry>;
pub type HeaderCas<'env> = CasBuffer<'env, ChainHeader>;

/// A convenient pairing of two CasBuffers, one for entries and one for headers
pub struct ChainCasBuffer<'env> {
    entries: EntryCas<'env>,
    headers: HeaderCas<'env>,
}

impl<'env> ChainCasBuffer<'env> {

    fn new(reader: &'env Reader<'env>, entries_store: SingleStore, headers_store: SingleStore) -> WorkspaceResult<Self> {
        Ok(Self {
            entries: CasBuffer::new(reader, entries_store)?,
            headers: CasBuffer::new(reader, headers_store)?,
        })
    }

    pub fn primary(reader: &'env Reader<'env>, dbm: &'env DbManager<'env>) -> WorkspaceResult<Self> {
        let entries = dbm.get(&*CHAIN_ENTRIES)?.clone();
        let headers = dbm.get(&*CHAIN_HEADERS)?.clone();
        Self::new(reader, entries, headers)
    }

    pub fn get_entry(&self, entry_address: &Address) -> WorkspaceResult<Option<Entry>> {
        self.entries.get(entry_address)
    }

    pub fn get_header(&self, header_address: &Address) -> WorkspaceResult<Option<ChainHeader>> {
        self.headers.get(header_address)
    }

    /// Given a ChainHeader, return the corresponding HeaderWithEntry
    pub fn header_with_entry(&self, header: ChainHeader) -> SourceChainResult<Option<HeaderWithEntry>> {
        if let Some(entry) = self.get_entry(header.entry_address())? {
            Ok(Some(HeaderWithEntry::new(header, entry)))
        } else {
            Err(SourceChainError::InvalidStructure(
                ChainInvalidReason::MissingData(header.entry_address().clone()),
            ))
        }
    }

    pub fn get_header_with_entry(&self, header_address: &Address) -> SourceChainResult<Option<HeaderWithEntry>> {
        if let Some(header) = self.get_header(header_address)? {
            self.header_with_entry(header)
        } else {
            Ok(None)
        }
    }

    pub fn put(&mut self, v: (ChainHeader, Entry)) -> () {
        let (header, entry) = v;
        self.entries.put(entry);
        self.headers.put(header);
    }

    /// TODO: consolidate into single delete which handles entry and header together
    pub fn delete_entry(&mut self, k: Address) -> () {
        self.entries.delete(k)
    }

    pub fn delete_header(&mut self, k: Address) -> () {
        self.headers.delete(k)
    }

    pub fn headers(&self) -> &HeaderCas<'env> {
        &self.headers
    }
}

impl<'env> StoreBuffer<'env> for ChainCasBuffer<'env> {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        self.entries.finalize(writer)?;
        self.headers.finalize(writer)?;
        Ok(())
    }
}
