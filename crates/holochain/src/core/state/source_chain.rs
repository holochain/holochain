//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentPubKey,
//! cannot fail, so the function return types reflect that.

use holo_hash::*;
use holochain_keystore::Signature;
use holochain_state::{
    db::GetDb,
    error::DatabaseResult,
    prelude::{Readable, Reader},
};
use holochain_types::{composite_hash::HeaderAddress, entry::Entry, prelude::*, Header, HeaderHashed};
use shrinkwraprs::Shrinkwrap;

pub use error::*;
pub use source_chain_buffer::*;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct SourceChain<'env, R: Readable = Reader<'env>>(pub SourceChainBuf<'env, R>);

impl<'env, R: Readable> SourceChain<'env, R> {
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
    pub fn new(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::new(reader, dbs)?.into())
    }

    pub fn into_inner(self) -> SourceChainBuf<'env, R> {
        self.0
    }
}

impl<'env, R: Readable> From<SourceChainBuf<'env, R>> for SourceChain<'env, R> {
    fn from(buffer: SourceChainBuf<'env, R>) -> Self {
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

    /// Access the Entry portion of this triple.
    pub fn entry(&self) -> &Option<Entry> {
        &self.maybe_entry
    }
}

/// the header and the signature that signed it
#[derive(Clone, Debug, PartialEq)]
pub struct SignedHeaderHashed {
    header: HeaderHashed,
    signature: Signature,
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
