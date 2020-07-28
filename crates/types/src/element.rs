//! Defines a Element, the basic unit of Holochain data.

use crate::{
    header::{WireDelete, WireNewEntryHeader},
    prelude::*,
    HeaderHashed,
};
use futures::future::FutureExt;
use holochain_keystore::KeystoreError;
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::element::*;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::header::{EntryType, Header};
use must_future::MustBoxFuture;
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Element without the hashes for sending across the network
pub struct WireElement {
    /// The signed header for this element
    signed_header: SignedHeader,
    /// If there is an entry associated with this header it will be her
    maybe_entry: Option<Entry>,
    /// If this element is deleted then we require a single delete
    /// in the cache as proof of the tombstone
    deleted: Option<WireDelete>,
}

/// Responses from a dht get.
/// These vary is size depending on the level of metadata required
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GetElementResponse {
    /// Can be combined with any other metadata monotonically
    GetEntryFull(Option<Box<RawGetEntryResponse>>),
    /// Placeholder for more optimized get
    GetEntryPartial,
    /// Placeholder for more optimized get
    GetEntryCollapsed,
    /// Get a single element
    /// Can be combined with other metadata monotonically
    GetHeader(Option<Box<WireElement>>),
}

/// This type gives full metadata that can be combined
/// monotonically with other metadata and the actual data
// in the most compact way that also avoids multiple calls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct RawGetEntryResponse {
    /// The live headers from this authority.
    /// These can be collapsed to NewEntryHeaderLight
    /// Which omits the EntryHash and EntryType,
    /// saving 32 bytes each
    pub live_headers: BTreeSet<WireNewEntryHeader>,
    /// just the hashes of headers to delete
    // TODO: Perf could just send the HeaderHash of the
    // header being deleted but we would need to only ever store
    // if there was a header delete in our MetadataBuf and
    // not the delete header hash as we do now.
    // TODO: We should think about getting the whole ElementDelete
    // so we can validate the delete hash is correct
    pub deletes: HashSet<WireDelete>,
    /// The entry shared across all headers
    pub entry: Entry,
    /// The entry_type shared across all headers
    pub entry_type: EntryType,
    /// The entry hash shared across all headers
    pub entry_hash: EntryHash,
}

impl RawGetEntryResponse {
    /// Creates the response from a set of chain elements
    /// that share the same entry with any deletes.
    /// Note: It's the callers responsibility to check that
    /// elements all have the same entry. This is not checked
    /// due to the performance cost.
    /// ### Panics
    /// If the elements are not a header of EntryCreate or EntryDelete
    /// or there is no entry or the entry hash is different
    pub fn from_elements<E>(elements: E, deletes: HashSet<WireDelete>) -> Option<Self>
    where
        E: IntoIterator<Item = Element>,
    {
        let mut elements = elements.into_iter();
        elements.next().map(|element| {
            let mut live_headers = BTreeSet::new();
            let (new_entry_header, entry_type, entry, entry_hash) = Self::from_element(element);
            live_headers.insert(new_entry_header);
            let r = Self {
                live_headers,
                deletes,
                entry,
                entry_type,
                entry_hash,
            };
            elements.fold(r, |mut response, element| {
                let (new_entry_header, entry_type, entry, entry_hash) = Self::from_element(element);
                debug_assert_eq!(response.entry, entry);
                debug_assert_eq!(response.entry_type, entry_type);
                debug_assert_eq!(response.entry_hash, entry_hash);
                response.live_headers.insert(new_entry_header);
                response
            })
        })
    }

    fn from_element(element: Element) -> (WireNewEntryHeader, EntryType, Entry, EntryHash) {
        let (shh, entry) = element.into_inner();
        let entry = entry.expect("Get entry responses cannot be created without entries");
        let (header, signature) = shh.into_header_and_signature();
        let (new_entry_header, entry_type, entry_hash) = match header.into_content() {
            Header::EntryCreate(ec) => {
                let et = ec.entry_type.clone();
                let eh = ec.entry_hash.clone();
                (WireNewEntryHeader::Create((ec, signature).into()), et, eh)
            }
            Header::EntryUpdate(eu) => {
                let eh = eu.entry_hash.clone();
                let et = eu.entry_type.clone();
                (WireNewEntryHeader::Update((eu, signature).into()), et, eh)
            }
            h @ _ => panic!(
                "Get entry responses cannot be created from headers
                    other then EntryCreate or EntryUpdate.
                    Tried to with: {:?}",
                h
            ),
        };
        (new_entry_header, entry_type, entry, entry_hash)
    }
}

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait ElementExt {
    /// Validate the signature matches the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[async_trait::async_trait]
impl ElementExt for Element {
    /// Validates a chain element
    async fn validate(&self) -> Result<(), KeystoreError> {
        self.signed_header().validate().await?;

        //TODO: make sure that any cases around entry existence are valid:
        //      SourceChainError::InvalidStructure(HeaderAndEntryMismatch(address)),
        Ok(())
    }
}

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait SignedHeaderHashedExt {
    /// Create a hash from data
    fn from_content(signed_header: SignedHeader) -> MustBoxFuture<'static, SignedHeaderHashed>;
    /// Sign some content
    async fn new(
        keystore: &KeystoreSender,
        header: HeaderHashed,
    ) -> Result<SignedHeaderHashed, KeystoreError>;
    /// Validate the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[allow(missing_docs)]
#[async_trait::async_trait]
impl SignedHeaderHashedExt for SignedHeaderHashed {
    fn from_content(signed_header: SignedHeader) -> MustBoxFuture<'static, Self>
    where
        Self: Sized,
    {
        async move {
            let (header, signature) = signed_header.into();
            Self::with_presigned(HeaderHashed::from_content(header).await, signature)
        }
        .boxed()
        .into()
    }
    /// SignedHeader constructor
    async fn new(keystore: &KeystoreSender, header: HeaderHashed) -> Result<Self, KeystoreError> {
        let signature = header.author().sign(keystore, &*header).await?;
        Ok(Self::with_presigned(header, signature))
    }

    /// Validates a signed header
    async fn validate(&self) -> Result<(), KeystoreError> {
        if !self
            .header()
            .author()
            .verify_signature(self.signature(), self.header())
            .await?
        {
            return Err(KeystoreError::InvalidSignature(
                self.signature().clone(),
                format!("header {:?}", self.header_address()),
            ));
        }
        Ok(())
    }
}

impl WireElement {
    /// Has this element been deleted according to the authority
    pub fn deleted(&self) -> &Option<WireDelete> {
        &self.deleted
    }

    /// Convert into a [Element] when receiving from the network
    pub async fn into_element(self) -> Element {
        Element::new(
            SignedHeaderHashed::from_content(self.signed_header).await,
            self.maybe_entry,
        )
    }
    /// Convert from a [Element] when sending to the network
    pub fn from_element(e: Element, deleted: Option<WireDelete>) -> Self {
        let (signed_header, maybe_entry) = e.into_inner();
        Self {
            signed_header: signed_header.into_inner().0,
            maybe_entry: maybe_entry,
            deleted,
        }
    }

    /// Get the entry hash if there is one
    pub fn entry_hash(&self) -> Option<&EntryHash> {
        self.signed_header
            .header()
            .entry_data()
            .map(|(hash, _)| hash)
    }
}

#[cfg(test)]
mod tests {
    use super::{SignedHeader, SignedHeaderHashed};
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use holo_hash::{HasHash, HoloHashed};

    #[tokio::test(threaded_scheduler)]
    async fn test_signed_header_roundtrip() {
        let signature = SignatureFixturator::new(Unpredictable).next().unwrap();
        let header = HeaderFixturator::new(Unpredictable).next().unwrap();
        let signed_header = SignedHeader(header, signature);
        let hashed: HoloHashed<SignedHeader> = HoloHashed::from_content(signed_header).await;
        let shh: SignedHeaderHashed = hashed.clone().into();

        assert_eq!(shh.header_address(), hashed.as_hash());

        let round: HoloHashed<SignedHeader> = shh.into();

        assert_eq!(hashed, round);
    }
}
