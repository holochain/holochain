//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentPubKey,
//! cannot fail, so the function return types reflect that.

use holo_hash::*;
use holochain_keystore::Signature;
use holochain_state::{db::DbManager, error::DatabaseResult, prelude::Readable};
use holochain_types::{address::HeaderAddress, entry::Entry, prelude::*, ChainHeader};
use shrinkwraprs::Shrinkwrap;

pub use error::*;
pub use source_chain_buffer::*;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
pub struct SourceChain<'env, R: Readable>(SourceChainBuf<'env, R>);

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
    pub fn new(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::new(reader, dbs)?.into())
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
    signed_header: SignedHeader,
    maybe_entry: Option<Entry>,
}

impl ChainElement {
    /// Raw element constructor.  Used only when we know that the values are valid.
    pub fn new(signature: Signature, header: ChainHeader, maybe_entry: Option<Entry>) -> Self {
        Self {
            signed_header: SignedHeader { signature, header },
            maybe_entry,
        }
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

    /// Access the ChainHeader portion of this triple.
    pub fn header(&self) -> &ChainHeader {
        self.signed_header.header()
    }

    /// Access the Entry portion of this triple.
    pub fn entry(&self) -> &Option<Entry> {
        &self.maybe_entry
    }
}

/// the header and the signature that signed it
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SignedHeader {
    header: ChainHeader,
    signature: Signature,
}

impl SignedHeader {
    /// SignedHeader constructor
    pub async fn new(keystore: &KeystoreSender, header: ChainHeader) -> SourceChainResult<Self> {
        let signature = header.author().sign(keystore, &header).await?;
        Ok(Self { signature, header })
    }

    /// Access the ChainHeader portion.
    pub fn header(&self) -> &ChainHeader {
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
            .verify_signature(&self.signature, &self.header)
            .await?
        {
            return Err(SourceChainError::InvalidSignature);
        }
        Ok(())
    }
}
