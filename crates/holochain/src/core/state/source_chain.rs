//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentPubKey,
//! cannot fail, so the function return types reflect that.

use holo_hash::*;
use holochain_keystore::Signature;
use holochain_state::{db::DbManager, error::DatabaseResult, prelude::{Readable, Reader}};
use holochain_types::{
    address::HeaderAddress, chain_header::ChainHeader, entry::Entry, prelude::*,
};
use shrinkwraprs::Shrinkwrap;

pub use error::*;
pub use source_chain_buffer::*;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct SourceChain<'env, R: Readable>(pub SourceChainBuf<'env, R>);

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
pub mod raw {
    use super::*;
    // TODO write tests to varify the invariant.
    /// This is needed to use the database where
    /// the lifetimes cannot be verified by
    /// the compiler (e.g. with wasmer).
    /// The checks are moved to runtime.
    /// The api is non-blocking because this
    /// should never be contested if the invariant is held.
    /// This type cannot write to the db.
    /// It only takes a [Reader].
    pub struct UnsafeSourceChain {
        source_chain: std::sync::Weak<std::sync::RwLock<*mut std::ffi::c_void>>,
    }

    // TODO: SAFETY: Tie the guard to the lmdb `'env` lifetime.
    /// If this guard is dropped the underlying
    /// ptr cannot be used.
    /// ## Safety
    /// Don't use `mem::forget` on this type as it will
    /// break the checks.
    pub struct UnsafeSourceChainGuard {
        source_chain: Option<std::sync::Arc<std::sync::RwLock<*mut std::ffi::c_void>>>,
    }

    impl UnsafeSourceChain {
        pub fn from_mut(source_chain: &mut SourceChain<Reader>) -> (UnsafeSourceChainGuard, Self) {
            let raw_ptr = source_chain as *mut SourceChain<Reader> as *mut std::ffi::c_void;
            let guard = std::sync::Arc::new(std::sync::RwLock::new(raw_ptr));
            let source_chain = std::sync::Arc::downgrade(&guard);
            let guard = UnsafeSourceChainGuard {
                source_chain: Some(guard),
            };
            let source_chain = Self { source_chain };
            (guard, source_chain)
        }

        #[cfg(test)]
        /// Useful when we need this type for tests where we don't want to use it.
        /// It will always return None.
        pub fn test() -> Self {
            let fake_ptr = std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();
            let guard = std::sync::Arc::new(std::sync::RwLock::new(fake_ptr));
            let source_chain = std::sync::Arc::downgrade(&guard);
            // Make sure the weak Arc cannot be upgraded
            std::mem::drop(guard);
            Self { source_chain }
        }

        pub unsafe fn apply_ref<R: 'static, F: FnOnce(&SourceChain<Reader>) -> R>(
            &self,
            f: F,
        ) -> Option<R> {
            // Check it exists
            self.source_chain
                .upgrade()
                // Check that no-one else can write
                .and_then(|lock| {
                    lock.try_read().ok().and_then(|guard| {
                        let sc = *guard as *const SourceChain<Reader>;
                        sc.as_ref().map(|s| f(s))
                    })
                })
        }

        pub unsafe fn apply_mut<R, F: FnOnce(&mut SourceChain<Reader>) -> R>(
            &self,
            f: F,
        ) -> Option<R> {
            // Check it exists
            self.source_chain
                .upgrade()
                // Check that no-one else can read or write
                .and_then(|lock| {
                    lock.try_write().ok().and_then(|guard| {
                        let sc = *guard as *mut SourceChain<Reader>;
                        sc.as_mut().map(|s| f(s))
                    })
                })
        }
    }

    impl Drop for UnsafeSourceChainGuard {
        fn drop(&mut self) {
            std::sync::Arc::try_unwrap(self.source_chain.take().expect("BUG: This has to be here"))
                .expect("BUG: Invariant broken, strong reference active while guard is dropped");
        }
    }
}
