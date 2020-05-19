use crate::core::state::source_chain::{
    ChainElement, ChainInvalidReason, SignedHeaderHashed, SourceChainError, SourceChainResult,
};
use holo_hash::{EntryHash, Hashed, HeaderHash};
use holochain_state::{
    buffer::{BufferedStore, CasBuf},
    db::{
        GetDb, CACHE_CHAIN_ENTRIES, CACHE_CHAIN_HEADERS, PRIMARY_CHAIN_ENTRIES_PRIVATE,
        PRIMARY_CHAIN_ENTRIES_PUBLIC, PRIMARY_CHAIN_HEADERS,
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

pub type EntryCas<'env, R> = CasBuf<'env, Entry, R>;
pub type HeaderCas<'env, R> = CasBuf<'env, (Header, Signature), R>;

/// A convenient pairing of two CasBufs, one for entries and one for headers
pub struct ChainCasBuf<'env, R: Readable = Reader<'env>> {
    entries_public: EntryCas<'env, R>,
    entries_private: Option<EntryCas<'env, R>>,
    headers: HeaderCas<'env, R>,
}

impl<'env, R: Readable> ChainCasBuf<'env, R> {
    fn new(
        reader: &'env R,
        entries_public_store: SingleStore,
        entries_private_store: Option<SingleStore>,
        headers_store: SingleStore,
    ) -> DatabaseResult<Self> {
        let entries_private = if let Some(store) = entries_private_store {
            Some(CasBuf::new(reader, store)?)
        } else {
            None
        };
        Ok(Self {
            entries_public: CasBuf::new(reader, entries_public_store)?,
            entries_private,
            headers: CasBuf::new(reader, headers_store)?,
        })
    }

    pub fn primary(reader: &'env R, dbs: &impl GetDb, allow_private: bool) -> DatabaseResult<Self> {
        let headers = dbs.get_db(&*PRIMARY_CHAIN_HEADERS)?;
        let entries = dbs.get_db(&*PRIMARY_CHAIN_ENTRIES_PUBLIC)?;
        let entries_private = if allow_private {
            Some(dbs.get_db(&*PRIMARY_CHAIN_ENTRIES_PRIVATE)?)
        } else {
            None
        };
        Self::new(reader, entries, entries_private, headers)
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
        self.entries_public.get(&entry_address.into())
    }

    /// Get an entry from the private DB if specified, else always return None
    /// TODO: maybe expose publicly if it makes sense (it is safe to do so)
    fn get_private_entry(&self, entry_address: EntryAddress) -> DatabaseResult<Option<Entry>> {
        if let Some(ref db) = self.entries_private {
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
                    self.entries_public.put(entry_address.into(), entry);
                } else {
                    self.entries_private
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
        self.entries_public.delete(entry_hash.clone().into());
        self.entries_private
            .as_mut()
            .map(|db| db.delete(entry_hash.into()));
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.headers
    }

    pub fn public_entries(&self) -> &EntryCas<'env, R> {
        &self.entries_public
    }
}

impl<'env, R: Readable> BufferedStore<'env> for ChainCasBuf<'env, R> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.entries_public.flush_to_txn(writer)?;
        if let Some(db) = self.entries_private {
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
