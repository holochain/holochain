use crate::prelude::*;

/// Anything that can go wrong while calling a HostFnApi method
#[derive(thiserror::Error, Debug)]
pub enum ZomeError {
    /// ZomeNotFound
    #[error("Zome not found: {0}")]
    ZomeNotFound(String),

    /// NonWasmZome
    #[error("Accessed a zome expecting to find a WasmZome, but found other type. Zome name: {0}")]
    NonWasmZome(ZomeName),

    /// SerializedBytesError (can occur during DnaDef::update_modifiers)
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    #[error("Zome dependency {0} for {1} is not pointing at an existing integrity zome that is not itself")]
    DanglingZomeDependency(String, String),
}

pub type ZomeResult<T> = Result<T, ZomeError>;
