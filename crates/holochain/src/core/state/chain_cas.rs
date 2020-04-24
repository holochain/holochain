use crate::core::state::source_chain::{ChainInvalidReason, SourceChainError, SourceChainResult};
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{BufferedStore, CasBuf},
    db::{
        DbManager, CACHE_CHAIN_ENTRIES, CACHE_CHAIN_HEADERS, PRIMARY_CHAIN_ENTRIES,
        PRIMARY_CHAIN_HEADERS,
    },
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    prelude::{Readable, Reader, Writer},
};
use holochain_types::{
    chain_header::HeaderAddress,
    chain_header::{ChainHeader, HeaderWithEntry},
    entry::Entry,
    entry::EntryAddress,
};

pub type EntryCas<'env, R> = CasBuf<'env, Entry, R>;
pub type HeaderCas<'env, R> = CasBuf<'env, ChainHeader, R>;

/// A convenient pairing of two CasBufs, one for entries and one for headers
pub struct ChainCasBuf<'env, R: Readable = Reader<'env>> {
    entries: EntryCas<'env, R>,
    headers: HeaderCas<'env, R>,
}

impl<'env, R: Readable> ChainCasBuf<'env, R> {
    fn new(
        reader: &'env R,
        entries_store: SingleStore,
        headers_store: SingleStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            entries: CasBuf::new(reader, entries_store)?,
            headers: CasBuf::new(reader, headers_store)?,
        })
    }

    pub fn primary(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        let entries = *dbs.get(&*PRIMARY_CHAIN_ENTRIES)?;
        let headers = *dbs.get(&*PRIMARY_CHAIN_HEADERS)?;
        Self::new(reader, entries, headers)
    }

    pub fn cache(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        let entries = *dbs.get(&*CACHE_CHAIN_ENTRIES)?;
        let headers = *dbs.get(&*CACHE_CHAIN_HEADERS)?;
        Self::new(reader, entries, headers)
    }

    pub fn get_entry(&self, entry_address: EntryAddress) -> DatabaseResult<Option<Entry>> {
        self.entries.get(&entry_address.into())
    }

    pub fn contains(&self, entry_address: EntryAddress) -> DatabaseResult<bool> {
        self.entries.get(&entry_address.into()).map(|e| e.is_some())
    }

    pub fn get_header(&self, header_address: HeaderAddress) -> DatabaseResult<Option<ChainHeader>> {
        self.headers.get(&header_address.into())
    }

    // Given a SignedHeader, return the corresponding ChainElement
    fn chain_element(
        &self,
        signed_header: SignedHeader,
    ) -> SourceChainResult<Option<ChainElement>> {
        maybe_entry = signed_header.header.entry_address().map(|entry_address| {
            // if the header has an address it better have been stored!
            let maybe_entry = self.get_entry(entry_address())?;
            if maybe_entry.is_none() {
                return Err(SourceChainError::InvalidStructure(
                    ChainInvalidReason::MissingData(header.entry_address()),
                ));
            }
            maybe_entry
        });
        ChainElement::new(signed_header.signature, signed_header.header, maybe_entry);
    }

    /// given a header address return the full chain element for that address
    pub fn get_element(
        &self,
        header_address: &HeaderAddress,
    ) -> SourceChainResult<Option<ChainElement>> {
        if let Some(singed_header) = self.get_header(header_address.to_owned())? {
            self.chain_element(signed_header)
        } else {
            Ok(None)
        }
    }

    pub fn put(&mut self, v: ChainElement) -> DatabaseResult<()> {
        let (signature, header, maybe_entry) = v;
        let signed_header = SignedHeader { signature, header };
        if is_some(maybe_entry) {
            self.entries.put((&entry).try_into()?, entry);
        }
        self.headers.put((&header).try_into()?, signed_header);
        Ok(())
    }

    // TODO: consolidate into single delete which handles full element deleted together
    pub fn delete_entry(&mut self, k: EntryHash) {
        self.entries.delete(k.into())
    }

    pub fn delete_header(&mut self, k: HeaderHash) {
        self.headers.delete(k.into())
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.headers
    }

    pub fn entries(&self) -> &EntryCas<'env, R> {
        &self.entries
    }
}

impl<'env, R: Readable> BufferedStore<'env> for ChainCasBuf<'env, R> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.entries.flush_to_txn(writer)?;
        self.headers.flush_to_txn(writer)?;
        Ok(())
    }
}
