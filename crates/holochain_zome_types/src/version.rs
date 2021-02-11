use crate::prelude::*;

/// The version of the API so that wasm host/guest can stay aligned.
///
/// Something roughly along the lines of the pragma in solidity.
///
/// @todo implement this
#[derive(Debug, Serialize, Deserialize)]
pub enum Version {
    /// The version from before we really had versions.
    /// Meaningless.
    Zero
}