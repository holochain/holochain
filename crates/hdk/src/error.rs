use thiserror::Error;

#[derive(Debug, Error)]
pub enum HdkError {
    #[error(transparent)]
    EntryError(#[from] holochain_zome_types::entry::EntryError),

    #[error(transparent)]
    SerializedBytes(#[from] holochain_wasmer_guest::SerializedBytesError),

    #[error(transparent)]
    Wasm(#[from] holochain_wasmer_guest::WasmError),

    #[error("Zome call was made which the caller was unauthorized to make")]
    UnauthorizedZomeCall,
}
