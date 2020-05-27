//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentPubKey,
//! cannot fail, so the function return types reflect that.

use derive_more::{From, Into};
use futures::future::{BoxFuture, FutureExt};
use holo_hash::*;
use holochain_keystore::Signature;
use holochain_state::{
    db::GetDb,
    error::DatabaseResult,
    prelude::{Readable, Reader},
};
use holochain_types::{
    composite_hash::HeaderAddress, header::EntryVisibility, prelude::*, Header, HeaderHashed,
};
use holochain_zome_types::entry::Entry;
use shrinkwraprs::Shrinkwrap;

pub use error::*;
pub use source_chain_buffer::*;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct SourceChain<'env>(pub SourceChainBuf<'env>);

impl<'env> SourceChain<'env> {
    pub fn agent_pubkey(&self) -> SourceChainResult<AgentPubKey> {
        self.0
            .agent_pubkey()?
            .ok_or(SourceChainError::InvalidStructure(
                ChainInvalidReason::GenesisDataMissing,
            ))
    }

    pub fn chain_head(&self) -> SourceChainResult<&HeaderAddress> {
        self.0.chain_head().ok_or(SourceChainError::ChainEmpty)
    }
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::new(reader, dbs)?.into())
    }

    pub fn into_inner(self) -> SourceChainBuf<'env> {
        self.0
    }
}

impl<'env> From<SourceChainBuf<'env>> for SourceChain<'env> {
    fn from(buffer: SourceChainBuf<'env>) -> Self {
        Self(buffer)
    }
}

/// a chain element which is a triple containing the signature of the header along with the
/// entry if the header type has one.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainElement {
    signed_header: SignedHeaderHashed,
    maybe_entry: Option<Entry>,
}

impl ChainElement {
    /// Raw element constructor.  Used only when we know that the values are valid.
    pub fn new(signed_header: SignedHeaderHashed, maybe_entry: Option<Entry>) -> Self {
        Self {
            signed_header,
            maybe_entry,
        }
    }

    pub fn into_inner(self) -> (SignedHeaderHashed, Option<Entry>) {
        (self.signed_header, self.maybe_entry)
    }

    /// Validates a chain element
    pub async fn validate(&self) -> SourceChainResult<()> {
        self.signed_header.validate().await?;

        //TODO: make sure that any cases around entry existence are valid:
        //      SourceChainError::InvalidStructure(HeaderAndEntryMismatch(address)),
        Ok(())
    }

    /// Access the signature portion of this triple.
    pub fn signature(&self) -> &Signature {
        self.signed_header.signature()
    }

    /// Access the header address
    pub fn header_address(&self) -> &HeaderAddress {
        self.signed_header.header_address()
    }

    /// Access the Header portion of this triple.
    pub fn header(&self) -> &Header {
        self.signed_header.header()
    }

    /// Access the HeaderHashed portion.
    pub fn header_hashed(&self) -> &HeaderHashed {
        self.signed_header.header_hashed()
    }

    /// Access the Entry portion of this triple as a ChainElementEntry,
    /// which includes the context around the presence or absence of the entry.
    pub fn entry(&self) -> ChainElementEntry {
        let maybe_visibilty = self
            .header()
            .entry_data()
            .map(|(_, entry_type)| entry_type.visibility());
        match (self.maybe_entry.as_ref(), maybe_visibilty) {
            (Some(entry), Some(_)) => ChainElementEntry::Present(entry),
            (None, Some(EntryVisibility::Private)) => ChainElementEntry::Hidden,
            (None, None) => ChainElementEntry::NotApplicable,
            (Some(_), None) => {
                unreachable!("Entry is present for a Header type which has no entry reference")
            }
            (None, Some(EntryVisibility::Public)) => unreachable!("Entry data missing for element"),
        }
    }
}

/// Represents the different ways the entry_address reference within a Header
/// can be intepreted
#[derive(Clone, Debug, PartialEq, Eq, derive_more::From)]
pub enum ChainElementEntry<'a> {
    /// The Header has an entry_address reference, and the Entry is accessible.
    Present(&'a Entry),
    /// The Header has an entry_address reference, but we are in a public
    /// context and the entry is private.
    Hidden,
    /// The Header does not contain an entry_address reference.
    NotApplicable,
}

impl<'a> ChainElementEntry<'a> {
    pub fn as_option(&'a self) -> Option<&'a Entry> {
        if let ChainElementEntry::Present(entry) = self {
            Some(entry)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, From, Into, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct SignedHeader(Header, Signature);

/// the header and the signature that signed it
#[derive(Clone, Debug, PartialEq)]
pub struct SignedHeaderHashed {
    header: HeaderHashed,
    signature: Signature,
}

impl Hashed for SignedHeaderHashed {
    type Content = SignedHeader;

    type HashType = HeaderHash;

    /// Unwrap the complete contents of this "Hashed" wrapper.
    fn into_inner(self) -> (Self::Content, Self::HashType) {
        let (header, hash) = self.header.into_inner();
        ((header, self.signature).into(), hash)
    }

    /// Access the main item stored in this wrapper type.
    fn as_content(&self) -> &Self::Content {
        todo!("figure out")
        // let header = self.header.as_content();
        // (header, &self.signature)
    }

    /// Access the already-calculated hash stored in this wrapper type.
    fn as_hash(&self) -> &Self::HashType {
        self.header.as_hash()
    }
}

impl Hashable for SignedHeaderHashed {
    fn with_data(
        signed_header: Self::Content,
    ) -> BoxFuture<'static, Result<Self, SerializedBytesError>>
    where
        Self: Sized,
    {
        async move {
            let (header, signature) = signed_header.into();
            Ok(Self {
                header: HeaderHashed::with_data(header).await?,
                signature,
            })
        }
        .boxed()
    }
}

impl SignedHeaderHashed {
    /// SignedHeader constructor
    pub async fn new(keystore: &KeystoreSender, header: HeaderHashed) -> SourceChainResult<Self> {
        let signature = header.author().sign(keystore, &*header).await?;
        Ok(Self::with_presigned(header, signature))
    }

    /// Constructor for an already signed header
    pub fn with_presigned(header: HeaderHashed, signature: Signature) -> Self {
        Self { header, signature }
    }

    pub fn into_inner(self) -> (HeaderHashed, Signature) {
        (self.header, self.signature)
    }

    /// Access the Header Hash.
    pub fn header_address(&self) -> &HeaderAddress {
        self.header.as_hash()
    }

    /// Access the Header portion.
    pub fn header(&self) -> &Header {
        &*self.header
    }

    /// Access the HeaderHashed portion.
    pub fn header_hashed(&self) -> &HeaderHashed {
        &self.header
    }

    /// Access the signature portion.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Validates a signed header
    pub async fn validate(&self) -> SourceChainResult<()> {
        if !self
            .header
            .author()
            .verify_signature(&self.signature, &*self.header)
            .await?
        {
            return Err(SourceChainError::InvalidSignature);
        }
        Ok(())
    }
}
