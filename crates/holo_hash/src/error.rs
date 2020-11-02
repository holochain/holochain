//! HoloHash Error Type.

/// HoloHash Error Type.
#[derive(Debug)]
pub enum HoloHashError {
    /// holo hashes begin with a lower case u (base64url_no_pad)
    NoU,

    /// could not base64 decode the holo hash
    BadBase64,

    /// this string is not the right size for a holo hash
    BadSize,

    /// this hash does not seem to match a known holo hash prefix
    BadPrefix,

    /// checksum validation failed
    BadChecksum,
}

pub type HoloHashResult<T> = Result<T, HoloHashError>;
