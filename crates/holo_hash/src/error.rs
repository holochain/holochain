//! HoloHash Error Type.

use crate::HOLO_HASH_PREFIX_LEN;

/// HoloHash Error Type.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum HoloHashError {
    /// holo hashes begin with a lower case u (base64url_no_pad)
    #[error("Holo Hash missing 'u' prefix")]
    NoU,

    /// could not base64 decode the holo hash
    #[error("Holo Hash has invalid base64 encoding")]
    BadBase64,

    /// this string is not the right size for a holo hash
    #[error("Holo Hash has incorrect size")]
    BadSize,

    /// this hash does not match a known holo hash prefix
    #[error("Holo Hash {0} has unknown prefix {1:?}")]
    BadPrefix(String, [u8; HOLO_HASH_PREFIX_LEN]),

    /// checksum validation failed
    #[error("Holo Hash checksum validation failed")]
    BadChecksum,

    /// this hash size is too large for blake2b
    #[error("Bad Blake2B hash size.")]
    BadHashSize,
}

/// HoloHash Result type
pub type HoloHashResult<T> = Result<T, HoloHashError>;
