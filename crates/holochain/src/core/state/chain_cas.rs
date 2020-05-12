use crate::core::state::source_chain::{
    ChainElement, ChainInvalidReason, SignedHeaderHashed, SourceChainError, SourceChainResult,
};
use holo_hash::{EntryHash, HeaderHash, Hashed};
use holochain_state::{
    buffer::{BufferedStore, CasBuf},
    db::{
        GetDb, CACHE_CHAIN_ENTRIES, CACHE_CHAIN_HEADERS, PRIMARY_CHAIN_ENTRIES,
        PRIMARY_CHAIN_HEADERS,
    },
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    prelude::{Readable, Reader, Writer},
};
use holochain_types::{
    address::{EntryAddress, HeaderAddress},
    entry::{Entry, EntryHashed},
    header,
    Header,
    HeaderHashed,
    prelude::{Signature},
};

pub type EntryCas<'env, R> = CasBuf<'env, Entry, R>;
pub type HeaderCas<'env, R> = CasBuf<'env, (Header, Signature), R>;

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

    pub fn primary(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let entries = dbs.get_db(&*PRIMARY_CHAIN_ENTRIES)?;
        let headers = dbs.get_db(&*PRIMARY_CHAIN_HEADERS)?;
        Self::new(reader, entries, headers)
    }

    pub fn cache(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let entries = dbs.get_db(&*CACHE_CHAIN_ENTRIES)?;
        let headers = dbs.get_db(&*CACHE_CHAIN_HEADERS)?;
        Self::new(reader, entries, headers)
    }

    pub fn get_entry(&self, entry_address: EntryAddress) -> DatabaseResult<Option<Entry>> {
        self.entries.get(&entry_address.into())
    }

    pub fn contains(&self, entry_address: EntryAddress) -> DatabaseResult<bool> {
        self.entries.get(&entry_address.into()).map(|e| e.is_some())
    }

    pub fn get_prev_header(
        &self,
        header_address: &HeaderAddress,
    ) -> DatabaseResult<Option<HeaderAddress>> {
        if let Ok(Some((header, _))) = self.headers.get(&header_address.to_owned().into()) {
            Ok(header.prev_header().map(|h| h.to_owned()))
        } else {
            Ok(None)
        }
    }

    pub async fn get_header(
        &self,
        header_address: &HeaderAddress,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        if let Ok(Some((header, signature))) = self.headers.get(&header_address.to_owned().into()) {
            let header = HeaderHashed::with_data(header).await?;
            assert_eq!(header_address, header.as_hash());
            Ok(Some(SignedHeaderHashed::with_presigned(header, signature)))
        } else {
            Ok(None)
        }
    }

    // local helper function which given a SignedHeaderHashed, looks for an entry in the cas
    // and builds a ChainElement struct
    fn chain_element(
        &self,
        signed_header: SignedHeaderHashed,
    ) -> SourceChainResult<Option<ChainElement>> {
        let maybe_entry_address = match signed_header.header() {
            Header::EntryCreate(header::EntryCreate { entry_address, .. }) => Some(entry_address.clone()),
            Header::EntryUpdate(header::EntryUpdate { entry_address, .. }) => Some(entry_address.clone()),
            _ => None,
        };
        let maybe_entry = match maybe_entry_address {
            None => None,
            Some(entry_address) => {
                // if the header has an address it better have been stored!
                let maybe_cas_entry = self.get_entry(entry_address.clone())?;
                if maybe_cas_entry.is_none() {
                    return Err(SourceChainError::InvalidStructure(
                        ChainInvalidReason::MissingData(entry_address),
                    ));
                }
                maybe_cas_entry
            }
        };
        Ok(Some(ChainElement::new(
            signed_header,
            maybe_entry,
        )))
    }

    /// given a header address return the full chain element for that address
    pub async fn get_element(
        &self,
        header_address: &HeaderAddress,
    ) -> SourceChainResult<Option<ChainElement>> {
        if let Some(signed_header) = self.get_header(header_address).await? {
            self.chain_element(signed_header)
        } else {
            Ok(None)
        }
    }

    /// Puts a signed header and optional entry onto the CAS.
    /// N.B. this code assumes that the header and entry have been validated
    pub fn put(
        &mut self,
        signed_header: SignedHeaderHashed,
        maybe_entry: Option<EntryHashed>,
    ) -> DatabaseResult<()> {
        if let Some(entry) = maybe_entry {
            let (entry, entry_address) = entry.into_inner_with_hash();
            self.entries.put(entry_address.into(), entry);
        }
        let (header, signature) = signed_header.into_inner();
        let (header, header_address) = header.into_inner_with_hash();
        self.headers
            .put(header_address.into(), (header, signature));
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
