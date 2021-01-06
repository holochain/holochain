use holo_hash::AgentPubKey;
use holochain_zome_types::prelude::*;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum HdkError {
    #[error(transparent)]
    EntryError(#[from] holochain_zome_types::entry::EntryError),

    #[error(transparent)]
    SerializedBytes(#[from] holochain_wasmer_guest::SerializedBytesError),

    #[error(transparent)]
    Wasm(#[from] holochain_wasmer_guest::WasmError),

    #[error("Zome call was made which the caller was unauthorized to make")]
    UnauthorizedZomeCall(CellId, ZomeName, FunctionName, AgentPubKey),

    #[error("A remote zome call was made but there was a network error: {0}")]
    ZomeCallNetworkError(String),
}

pub type HdkResult<T> = Result<T, HdkError>;

impl From<core::convert::Infallible> for HdkError {
    fn from(_: core::convert::Infallible) -> Self {
        unreachable!()
    }
}
