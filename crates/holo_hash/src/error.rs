//! HoloHash Error Type.

use crate::HOLO_HASH_PREFIX_LEN;

/// HoloHash Error Type.
#[derive(Debug)]
pub enum HoloHashError {
    /// holo hashes begin with a lower case u (base64url_no_pad)
    NoU,

    /// could not base64 decode the holo hash
    BadBase64,

    /// this string is not the right size for a holo hash
    BadSize,

    /// this hash does not match a known holo hash prefix
    BadPrefix(String, [u8; HOLO_HASH_PREFIX_LEN]),

    /// checksum validation failed
    BadChecksum,
}

/// HoloHash Result type
pub type HoloHashResult<T> = Result<T, HoloHashError>;
