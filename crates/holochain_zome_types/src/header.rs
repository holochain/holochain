use crate::timestamp::Timestamp;
pub use builder::HeaderBuilder;
pub use builder::HeaderBuilderCommon;
use conversions::WrongHeaderError;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use thiserror::Error;

pub use holochain_integrity_types::header::*;

#[cfg(any(test, feature = "test_utils"))]
pub use facts::*;

pub mod builder;
#[cfg(any(test, feature = "test_utils"))]
pub mod facts;

#[derive(Error, Debug)]
pub enum HeaderError {
    #[error("Tried to create a NewEntryHeader with a type that isn't a Create or Update")]
    NotNewEntry,
    #[error(transparent)]
    WrongHeaderError(#[from] WrongHeaderError),
    #[error("{0}")]
    Rebase(String),
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChainTopOrdering {
    /// Relaxed chain top ordering REWRITES HEADERS INLINE during a flush of
    /// the source chain to sit on top of the current chain top. The "as at"
    /// of the zome call initial state is completely ignored.
    /// This may be significantly more efficient if you are CERTAIN that none
    /// of your zome or validation logic is order dependent. Examples include
    /// simple chat messages or tweets. Note however that even chat messages
    /// and tweets may have subtle order dependencies, such as if a cap grant
    /// was written or revoked that would have invalidated the zome call that
    /// wrote data after the revocation, etc.
    /// The efficiency of relaxed ordering comes from simply rehashing and
    /// signing headers on the new chain top during flush, avoiding the
    /// overhead of the client, websockets, zome call instance, wasm execution,
    /// validation, etc. that would result from handling a `HeadMoved` error
    /// via an external driver.
    Relaxed,
    /// The default `Strict` ordering is the default for a very good reason.
    /// Writes normally compare the chain head from the start of a zome call
    /// against the time a write transaction is flushed from the source chain.
    /// This is REQUIRED for data integrity if any zome or validation logic
    /// depends on the ordering of data in a chain.
    /// This order dependence could be obvious such as an explicit reference or
    /// dependency. It could be very subtle such as checking for the existence
    /// or absence of some data.
    /// If you are unsure whether your data is order dependent you should err
    /// on the side of caution and handle `HeadMoved` errors on the client of
    /// the zome call and restart the zome call from the start.
    Strict,
}

impl Default for ChainTopOrdering {
    fn default() -> Self {
        Self::Strict
    }
}

pub trait HeaderExt {
    fn rebase_on(
        &mut self,
        new_prev_header: HeaderHash,
        new_prev_seq: u32,
        new_prev_timestamp: Timestamp,
    ) -> Result<(), HeaderError>;
}

impl HeaderExt for Header {
    fn rebase_on(
        &mut self,
        new_prev_header: HeaderHash,
        new_prev_seq: u32,
        new_prev_timestamp: Timestamp,
    ) -> Result<(), HeaderError> {
        let new_seq = new_prev_seq + 1;
        let new_timestamp = self.timestamp().max(
            (new_prev_timestamp + std::time::Duration::from_nanos(1))
                .map_err(|e| HeaderError::Rebase(e.to_string()))?,
        );
        match self {
            Self::Dna(_) => return Err(HeaderError::Rebase("Rebased a DNA Header".to_string())),
            Self::AgentValidationPkg(AgentValidationPkg {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::InitZomesComplete(InitZomesComplete {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::CreateLink(CreateLink {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::DeleteLink(DeleteLink {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::Delete(Delete {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::CloseChain(CloseChain {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::OpenChain(OpenChain {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::Create(Create {
                timestamp,
                header_seq,
                prev_header,
                ..
            })
            | Self::Update(Update {
                timestamp,
                header_seq,
                prev_header,
                ..
            }) => {
                *timestamp = new_timestamp;
                *header_seq = new_seq;
                *prev_header = new_prev_header;
            }
        };
        Ok(())
    }
}
