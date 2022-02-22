//! Defines a Element, the basic unit of Holochain data.

use crate::header::WireDelete;
use crate::header::WireHeaderStatus;
use crate::header::WireNewEntryHeader;
use crate::header::WireUpdateRelationship;
use crate::prelude::*;
use error::ElementGroupError;
use error::ElementGroupResult;
use holochain_keystore::KeystoreError;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::entry::EntryHashed;
use std::borrow::Cow;
use std::collections::BTreeSet;

#[allow(missing_docs)]
pub mod error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, Default)]
/// A condensed version of get element request.
/// This saves bandwidth by removing duplicated and implied data.
pub struct WireElementOps {
    /// The header this request was for.
    pub header: Option<Judged<SignedHeader>>,
    /// Any deletes on the header.
    pub deletes: Vec<Judged<WireDelete>>,
    /// Any updates on the header.
    pub updates: Vec<Judged<WireUpdateRelationship>>,
    /// The entry if there is one.
    pub entry: Option<Entry>,
}

impl WireElementOps {
    /// Create an empty set of wire element ops.
    pub fn new() -> Self {
        Self::default()
    }
    /// Render these ops to their full types.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            header,
            deletes,
            updates,
            entry,
        } = self;
        let mut ops = Vec::with_capacity(1 + deletes.len() + updates.len());
        if let Some(header) = header {
            let status = header.validation_status();
            let SignedHeader(header, signature) = header.data;
            // TODO: If they only need the metadata because they already have
            // the content we could just send the entry hash instead of the
            // SignedHeader.
            let entry_hash = header.entry_hash().cloned();
            ops.push(RenderedOp::new(
                header,
                signature,
                status,
                DhtOpType::StoreElement,
            )?);
            if let Some(entry_hash) = entry_hash {
                for op in deletes {
                    let status = op.validation_status();
                    let op = op.data;
                    let signature = op.signature;
                    let header = Header::Delete(op.delete);

                    ops.push(RenderedOp::new(
                        header,
                        signature,
                        status,
                        DhtOpType::RegisterDeletedBy,
                    )?);
                }
                for op in updates {
                    let status = op.validation_status();
                    let SignedHeader(header, signature) =
                        op.data.into_signed_header(entry_hash.clone());

                    ops.push(RenderedOp::new(
                        header,
                        signature,
                        status,
                        DhtOpType::RegisterUpdatedElement,
                    )?);
                }
            }
        }
        Ok(RenderedOps {
            entry: entry.map(EntryHashed::from_content_sync),
            ops,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Element without the hashes for sending across the network
/// TODO: Remove this as it's no longer needed.
pub struct WireElement {
    /// The signed header for this element
    signed_header: SignedHeader,
    /// If there is an entry associated with this header it will be here
    maybe_entry: Option<Entry>,
    /// The validation status of this element.
    validation_status: ValidationStatus,
    /// All deletes on this header
    deletes: Vec<WireHeaderStatus<WireDelete>>,
    /// Any updates on this entry.
    updates: Vec<WireHeaderStatus<WireUpdateRelationship>>,
}

/// A group of elements with a common entry
#[derive(Debug, Clone)]
pub struct ElementGroup<'a> {
    headers: Vec<Cow<'a, SignedHeaderHashed>>,
    rejected: Vec<Cow<'a, SignedHeaderHashed>>,
    entry: Cow<'a, EntryHashed>,
}

/// Element with it's status
#[derive(Debug, Clone, derive_more::Constructor)]
pub struct ElementStatus {
    /// The element this status applies to.
    pub element: Element,
    /// Validation status of this element.
    pub status: ValidationStatus,
}

impl<'a> ElementGroup<'a> {
    /// Get the headers and header hashes
    pub fn headers_and_hashes(&self) -> impl Iterator<Item = (&HeaderHash, &Header)> {
        self.headers
            .iter()
            .map(|shh| shh.header_address())
            .zip(self.headers.iter().map(|shh| shh.header()))
    }
    /// true if len is zero
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Amount of headers
    pub fn len(&self) -> usize {
        self.headers.len()
    }
    /// The entry's visibility
    pub fn visibility(&self) -> ElementGroupResult<&EntryVisibility> {
        self.headers
            .first()
            .ok_or(ElementGroupError::Empty)?
            .header()
            .entry_data()
            .map(|(_, et)| et.visibility())
            .ok_or(ElementGroupError::MissingEntryData)
    }
    /// The entry hash
    pub fn entry_hash(&self) -> &EntryHash {
        self.entry.as_hash()
    }
    /// The entry with hash
    pub fn entry_hashed(&self) -> EntryHashed {
        self.entry.clone().into_owned()
    }
    /// Get owned iterator of signed headers
    pub fn owned_signed_headers(&self) -> impl Iterator<Item = SignedHeaderHashed> + 'a {
        self.headers
            .clone()
            .into_iter()
            .chain(self.rejected.clone().into_iter())
            .map(|shh| shh.into_owned())
    }

    /// Get the valid header hashes
    pub fn valid_hashes(&self) -> impl Iterator<Item = &HeaderHash> {
        self.headers.iter().map(|shh| shh.header_address())
    }

    /// Get the rejected header hashes
    pub fn rejected_hashes(&self) -> impl Iterator<Item = &HeaderHash> {
        self.rejected.iter().map(|shh| shh.header_address())
    }

    /// Create an element group from wire headers and an entry
    pub fn from_wire_elements<I: IntoIterator<Item = WireHeaderStatus<WireNewEntryHeader>>>(
        headers_iter: I,
        entry_type: EntryType,
        entry: Entry,
    ) -> ElementGroupResult<ElementGroup<'a>> {
        let iter = headers_iter.into_iter();
        let mut valid = Vec::with_capacity(iter.size_hint().0);
        let mut rejected = Vec::with_capacity(iter.size_hint().0);
        let entry = entry.into_hashed();
        let entry_hash = entry.as_hash().clone();
        let entry = Cow::Owned(entry);
        for wire in iter {
            match wire.validation_status {
                ValidationStatus::Valid => valid.push(Cow::Owned(
                    wire.header
                        .into_header(entry_type.clone(), entry_hash.clone()),
                )),
                ValidationStatus::Rejected => rejected.push(Cow::Owned(
                    wire.header
                        .into_header(entry_type.clone(), entry_hash.clone()),
                )),
                ValidationStatus::Abandoned => todo!(),
            }
        }

        Ok(Self {
            headers: valid,
            rejected,
            entry,
        })
    }
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
    pub live_headers: BTreeSet<WireHeaderStatus<WireNewEntryHeader>>,
    /// just the hashes of headers to delete
    // TODO: Perf could just send the HeaderHash of the
    // header being deleted but we would need to only ever store
    // if there was a header delete in our MetadataBuf and
    // not the delete header hash as we do now.
    pub deletes: Vec<WireHeaderStatus<WireDelete>>,
    /// Any updates on this entry.
    /// Note you will need to ask for "all_live_headers_with_metadata"
    /// to get this back
    pub updates: Vec<WireHeaderStatus<WireUpdateRelationship>>,
    /// The entry shared across all headers
    pub entry: Entry,
    /// The entry_type shared across all headers
    pub entry_type: EntryType,
}

impl RawGetEntryResponse {
    /// Creates the response from a set of chain elements
    /// that share the same entry with any deletes.
    /// Note: It's the callers responsibility to check that
    /// elements all have the same entry. This is not checked
    /// due to the performance cost.
    /// ### Panics
    /// If the elements are not a header of Create or EntryDelete
    /// or there is no entry or the entry hash is different
    pub fn from_elements<E>(
        elements: E,
        deletes: Vec<WireHeaderStatus<WireDelete>>,
        updates: Vec<WireHeaderStatus<WireUpdateRelationship>>,
    ) -> Option<Self>
    where
        E: IntoIterator<Item = ElementStatus>,
    {
        let mut elements = elements.into_iter();
        elements.next().map(|ElementStatus { element, status }| {
            let mut live_headers = BTreeSet::new();
            let (new_entry_header, entry_type, entry) = Self::from_element(element);
            live_headers.insert(WireHeaderStatus::new(new_entry_header, status));
            let r = Self {
                live_headers,
                deletes,
                updates,
                entry,
                entry_type,
            };
            elements.fold(r, |mut response, ElementStatus { element, status }| {
                let (new_entry_header, entry_type, entry) = Self::from_element(element);
                debug_assert_eq!(response.entry, entry);
                debug_assert_eq!(response.entry_type, entry_type);
                response
                    .live_headers
                    .insert(WireHeaderStatus::new(new_entry_header, status));
                response
            })
        })
    }

    fn from_element(element: Element) -> (WireNewEntryHeader, EntryType, Entry) {
        let (shh, entry) = element.into_inner();
        let entry = entry
            .into_option()
            .expect("Get entry responses cannot be created without entries");
        let (header, signature) = shh.into_header_and_signature();
        let (new_entry_header, entry_type) = match header.into_content() {
            Header::Create(ec) => {
                let et = ec.entry_type.clone();
                (WireNewEntryHeader::Create((ec, signature).into()), et)
            }
            Header::Update(eu) => {
                let et = eu.entry_type.clone();
                (WireNewEntryHeader::Update((eu, signature).into()), et)
            }
            h => panic!(
                "Get entry responses cannot be created from headers
                    other then Create or Update.
                    Tried to with: {:?}",
                h
            ),
        };
        (new_entry_header, entry_type, entry)
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
    fn from_content_sync(signed_header: SignedHeader) -> SignedHeaderHashed;
    /// Sign some content
    #[allow(clippy::new_ret_no_self)]
    async fn sign(
        keystore: &MetaLairClient,
        header: HeaderHashed,
    ) -> LairResult<SignedHeaderHashed>;
    /// Validate the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[allow(missing_docs)]
#[async_trait::async_trait]
impl SignedHeaderHashedExt for SignedHeaderHashed {
    fn from_content_sync(signed_header: SignedHeader) -> Self
    where
        Self: Sized,
    {
        let (header, signature) = signed_header.into();
        Self::with_presigned(header.into_hashed(), signature)
    }
    /// SignedHeader constructor
    async fn sign(keystore: &MetaLairClient, header: HeaderHashed) -> LairResult<Self> {
        let signature = header.author().sign(keystore, &*header).await?;
        Ok(Self::with_presigned(header, signature))
    }

    /// Validates a signed header
    async fn validate(&self) -> Result<(), KeystoreError> {
        if !self
            .header()
            .author()
            .verify_signature(self.signature(), self.header())
            .await
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
    /// Convert into a [Element], deletes and updates when receiving from the network
    pub fn into_parts(self) -> (ElementStatus, Vec<ElementStatus>, Vec<ElementStatus>) {
        let entry_hash = self.signed_header.header().entry_hash().cloned();
        let header = Element::new(
            SignedHeaderHashed::from_content_sync(self.signed_header),
            self.maybe_entry,
        );
        let deletes = self
            .deletes
            .into_iter()
            .map(WireHeaderStatus::<WireDelete>::into_element_status)
            .collect();
        let updates = self
            .updates
            .into_iter()
            .map(|u| {
                let entry_hash = entry_hash
                    .clone()
                    .expect("Updates cannot be on headers that do not have entries");
                u.into_element_status(entry_hash)
            })
            .collect();
        (
            ElementStatus::new(header, self.validation_status),
            deletes,
            updates,
        )
    }
    /// Convert from a [Element] when sending to the network
    pub fn from_element(
        e: ElementStatus,
        deletes: Vec<WireHeaderStatus<WireDelete>>,
        updates: Vec<WireHeaderStatus<WireUpdateRelationship>>,
    ) -> Self {
        let ElementStatus { element, status } = e;
        let (signed_header, maybe_entry) = element.into_inner();
        Self {
            signed_header: signed_header.into_inner().0,
            // TODO: consider refactoring WireElement to use ElementEntry
            // instead of Option<Entry>
            maybe_entry: maybe_entry.into_option(),
            validation_status: status,
            deletes,
            updates,
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
    use super::SignedHeader;
    use super::SignedHeaderHashed;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use holo_hash::HasHash;
    use holo_hash::HoloHashed;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_signed_header_roundtrip() {
        let signature = SignatureFixturator::new(Unpredictable).next().unwrap();
        let header = HeaderFixturator::new(Unpredictable).next().unwrap();
        let signed_header = SignedHeader(header, signature);
        let hashed: HoloHashed<SignedHeader> = HoloHashed::from_content_sync(signed_header);
        let shh: SignedHeaderHashed = hashed.clone().into();

        assert_eq!(shh.header_address(), hashed.as_hash());

        let round: HoloHashed<SignedHeader> = shh.into();

        assert_eq!(hashed, round);
    }
}
