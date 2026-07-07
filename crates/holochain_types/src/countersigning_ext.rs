//! Countersigning type extensions for use on the Holochain host

use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::prelude::PreflightRequest;

/// Extensions to the [`PreflightRequest`] type.
pub trait PreflightRequestExt {
    /// Compute a fingerprint for this preflight request.
    fn fingerprint(&self) -> Result<Vec<u8>, SerializedBytesError>;
}

impl PreflightRequestExt for PreflightRequest {
    fn fingerprint(&self) -> Result<Vec<u8>, SerializedBytesError> {
        Ok(holo_hash::blake2b_256(
            &holochain_serialized_bytes::encode(self)?,
        ))
    }
}
