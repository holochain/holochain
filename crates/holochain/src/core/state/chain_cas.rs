/// A convenient composition of CasBufs, representing source chain data.
///
/// Source chain data is split into three databases: one for headers, and two
/// for public and private entries. Specifying the private_entries DB in a
/// ChainCasBuf is optional, so that if it's not supplied, the ChainCasBuf
/// will not be able to access private data. This restriction is useful when
/// using the ChainCasBuf for caching non-authored data, or for situations where
/// it is known that private entries should be protected, such as when handling
/// a get_entry request from the network.
use crate::core::state::source_chain::{
    ChainElement, ChainInvalidReason, SignedHeaderHashed, SourceChainError, SourceChainResult,
};
use holo_hash::{EntryHash, Hashed, HeaderHash};
use holochain_state::{
    buffer::{BufferedStore, CasBuf},
    db::{
        GetDb, CACHE_CHAIN_ENTRIES, CACHE_CHAIN_HEADERS, PRIMARY_CHAIN_HEADERS,
        PRIMARY_CHAIN_PRIVATE_ENTRIES, PRIMARY_CHAIN_PUBLIC_ENTRIES,
    },
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    prelude::{Readable, Reader, Writer},
};
use holochain_types::{
    address::{EntryAddress, HeaderAddress},
    entry::{Entry, EntryHashed},
    header,
    prelude::Signature,
    Header, HeaderHashed,
};

/// A CasBuf with Entries for values
pub type EntryCas<'env, R> = CasBuf<'env, Entry, R>;
/// A CasBuf with SignedHeaders for values
pub type HeaderCas<'env, R> = CasBuf<'env, (Header, Signature), R>;

/// The representation of a chain CAS, using two or three DB references
pub struct ChainCasBuf<'env, R: Readable = Reader<'env>> {
    public_entries: EntryCas<'env, R>,
    private_entries: Option<EntryCas<'env, R>>,
    headers: HeaderCas<'env, R>,
}

impl<'env, R: Readable> ChainCasBuf<'env, R> {
    fn new(
        reader: &'env R,
        public_entries_store: SingleStore,
        private_entries_store: Option<SingleStore>,
        headers_store: SingleStore,
    ) -> DatabaseResult<Self> {
        let private_entries = if let Some(store) = private_entries_store {
            Some(CasBuf::new(reader, store)?)
        } else {
            None
        };
        Ok(Self {
            public_entries: CasBuf::new(reader, public_entries_store)?,
            private_entries,
            headers: CasBuf::new(reader, headers_store)?,
        })
    }

    pub fn primary(reader: &'env R, dbs: &impl GetDb, allow_private: bool) -> DatabaseResult<Self> {
        let headers = dbs.get_db(&*PRIMARY_CHAIN_HEADERS)?;
        let entries = dbs.get_db(&*PRIMARY_CHAIN_PUBLIC_ENTRIES)?;
        let private_entries = if allow_private {
            Some(dbs.get_db(&*PRIMARY_CHAIN_PRIVATE_ENTRIES)?)
        } else {
            None
        };
        Self::new(reader, entries, private_entries, headers)
    }

    pub fn cache(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let entries = dbs.get_db(&*CACHE_CHAIN_ENTRIES)?;
        let headers = dbs.get_db(&*CACHE_CHAIN_HEADERS)?;
        Self::new(reader, entries, None, headers)
    }

    /// Get an entry by its address
    ///
    /// First attempt to get from the public entry DB. If not present, and
    /// private DB access is specified, attempt to get as a private entry.
    pub fn get_entry(&self, entry_address: EntryAddress) -> DatabaseResult<Option<Entry>> {
        match self.get_public_entry(entry_address.clone())? {
            Some(entry) => Ok(Some(entry)),
            None => self.get_private_entry(entry_address),
        }
    }

    /// Get an entry from the private DB if specified, else always return None
    /// TODO: maybe expose publicly if it makes sense (it is safe to do so)
    fn get_public_entry(&self, entry_address: EntryAddress) -> DatabaseResult<Option<Entry>> {
        self.public_entries.get(&entry_address.into())
    }

    /// Get an entry from the private DB if specified, else always return None
    /// TODO: maybe expose publicly if it makes sense (it is safe to do so)
    fn get_private_entry(&self, entry_address: EntryAddress) -> DatabaseResult<Option<Entry>> {
        if let Some(ref db) = self.private_entries {
            db.get(&entry_address.into())
        } else {
            Ok(None)
        }
    }

    pub fn contains(&self, entry_address: EntryAddress) -> DatabaseResult<bool> {
        self.get_entry(entry_address).map(|e| e.is_some())
    }

    pub async fn get_header(
        &self,
        header_address: &HeaderAddress,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        if let Ok(Some((header, signature))) = self.headers.get(&header_address.to_owned().into()) {
            let header = fatal_db_deserialize_check!(
                "ChainCasBuf::get_header",
                header_address,
                HeaderHashed::with_data(header).await,
            );
            fatal_db_hash_check!("ChainCasBuf::get_header", header_address, header.as_hash());
            Ok(Some(SignedHeaderHashed::with_presigned(header, signature)))
        } else {
            Ok(None)
        }
    }

    // local helper function which given a SignedHeaderHashed, looks for an entry in the cas
    // and builds a ChainElement struct
    fn get_element_inner(
        &self,
        signed_header: SignedHeaderHashed,
    ) -> SourceChainResult<Option<ChainElement>> {
        let maybe_entry_address = match signed_header.header() {
            Header::EntryCreate(header::EntryCreate {
                entry_address,
                entry_type,
                ..
            }) => Some((entry_address.clone(), entry_type.is_public())),
            Header::EntryUpdate(header::EntryUpdate {
                entry_address,
                entry_type,
                ..
            }) => Some((entry_address.clone(), entry_type.is_public())),
            _ => None,
        };
        let maybe_entry = match maybe_entry_address {
            None => None,
            Some((entry_address, is_public)) => {
                // if the header has an address it better have been stored!
                let maybe_cas_entry = if is_public {
                    self.get_public_entry(entry_address.clone())?
                } else {
                    self.get_private_entry(entry_address.clone())?
                };
                if maybe_cas_entry.is_none() {
                    return Err(SourceChainError::InvalidStructure(
                        ChainInvalidReason::MissingData(entry_address),
                    ));
                }
                maybe_cas_entry
            }
        };
        Ok(Some(ChainElement::new(signed_header, maybe_entry)))
    }

    /// given a header address return the full chain element for that address
    pub async fn get_element(
        &self,
        header_address: &HeaderAddress,
    ) -> SourceChainResult<Option<ChainElement>> {
        if let Some(signed_header) = self.get_header(header_address).await? {
            self.get_element_inner(signed_header)
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
        let (header, signature) = signed_header.into_inner();
        let (header, header_address) = header.into_inner();

        if let Some(entry) = maybe_entry {
            let (entry, entry_address) = entry.into_inner();
            if let Some(entry_type) = header.entry_type() {
                if entry_type.is_public() {
                    self.public_entries.put(entry_address.into(), entry);
                } else {
                    self.private_entries
                        .as_mut()
                        .map(|db| db.put(entry_address.into(), entry));
                }
            } else {
                unreachable!(
                    "Attempting to put an entry, but the header has no entry_type. Header hash: {}",
                    header_address
                );
            }
        }

        self.headers.put(header_address.into(), (header, signature));
        Ok(())
    }

    pub fn delete(&mut self, header_hash: HeaderHash, entry_hash: EntryHash) {
        self.headers.delete(header_hash.into());
        self.public_entries.delete(entry_hash.clone().into());
        self.private_entries
            .as_mut()
            .map(|db| db.delete(entry_hash.into()));
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.headers
    }

    pub fn public_entries(&self) -> &EntryCas<'env, R> {
        &self.public_entries
    }
}

impl<'env, R: Readable> BufferedStore<'env> for ChainCasBuf<'env, R> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.public_entries.flush_to_txn(writer)?;
        if let Some(db) = self.private_entries {
            db.flush_to_txn(writer)?
        };
        self.headers.flush_to_txn(writer)?;
        Ok(())
    }
}

mod tests {

    #[test]
    fn can_write_private_entry() {
        todo!("write plenty of tests")
    }
}
