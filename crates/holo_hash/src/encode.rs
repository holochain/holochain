use crate::assert_length;
use crate::error::HoloHashError;
use crate::HashType;
use crate::HoloHash;
use crate::PrimitiveHashType;
use crate::HOLO_HASH_FULL_LEN;
use crate::HOLO_HASH_PREFIX_LEN;
use std::convert::TryFrom;
use std::convert::TryInto;

#[cfg(feature = "hashing")]
pub use crate::hash_ext::{blake2b_128, blake2b_256, blake2b_n, holo_dht_location_bytes};

impl<P: PrimitiveHashType> TryFrom<&str> for HoloHash<P> {
    type Error = HoloHashError;
    fn try_from(s: &str) -> Result<Self, HoloHashError> {
        let hash_type = P::new();
        HoloHash::from_raw_39(holo_hash_decode(hash_type.get_prefix(), s)?)
    }
}

impl<P: PrimitiveHashType> TryFrom<&String> for HoloHash<P> {
    type Error = HoloHashError;
    fn try_from(s: &String) -> Result<Self, HoloHashError> {
        Self::try_from(s as &str)
    }
}

impl<P: PrimitiveHashType> TryFrom<String> for HoloHash<P> {
    type Error = HoloHashError;
    fn try_from(s: String) -> Result<Self, HoloHashError> {
        Self::try_from(&s)
    }
}

impl<T: HashType> std::fmt::Display for HoloHash<T> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(f, "{}", holo_hash_encode(self.get_raw_39()))
    }
}

/// internal REPR for holo hash
pub fn holo_hash_encode(data: &[u8]) -> String {
    format!("u{}", base64::encode_config(data, base64::URL_SAFE_NO_PAD),)
}

/// internal PARSE for holo hash REPR
pub fn holo_hash_decode_unchecked(s: &str) -> Result<Vec<u8>, HoloHashError> {
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    let decoded = match base64::decode_config(&s[1..], base64::URL_SAFE_NO_PAD) {
        Err(_) => return Err(HoloHashError::BadBase64),
        Ok(decoded) => decoded,
    };
    let hash_bytes: &[u8; HOLO_HASH_FULL_LEN] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| HoloHashError::BadSize)?;
    if !has_valid_checksum(&hash_bytes) {
        return Err(HoloHashError::BadChecksum);
    }
    assert_length!(HOLO_HASH_FULL_LEN, &decoded);
    Ok(decoded)
}

/// internal PARSE for holo hash REPR
pub fn holo_hash_decode(prefix: &[u8], s: &str) -> Result<Vec<u8>, HoloHashError> {
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    let decoded = match base64::decode_config(&s[1..], base64::URL_SAFE_NO_PAD) {
        Err(_) => return Err(HoloHashError::BadBase64),
        Ok(decoded) => decoded,
    };
    let hash_bytes: &[u8; HOLO_HASH_FULL_LEN] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| HoloHashError::BadSize)?;
    let actual_prefix = &hash_bytes[..HOLO_HASH_PREFIX_LEN];
    if actual_prefix != prefix {
        return Err(HoloHashError::BadPrefix(
            format!("{:?}", prefix),
            actual_prefix.try_into().unwrap(),
        ));
    }
    if !has_valid_checksum(hash_bytes) {
        return Err(HoloHashError::BadChecksum);
    }
    assert_length!(HOLO_HASH_FULL_LEN, &decoded);
    Ok(decoded)
}

#[cfg(feature = "hashing")]
fn has_valid_checksum(hash: &[u8; HOLO_HASH_FULL_LEN]) -> bool {
    use crate::HOLO_HASH_CORE_LEN;

    let expected = holo_dht_location_bytes(
        &hash[HOLO_HASH_PREFIX_LEN..HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN],
    );
    let actual = &hash[HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN..];
    expected == actual
}

#[cfg(not(feature = "hashing"))]
fn has_valid_checksum(_hash: &[u8; HOLO_HASH_FULL_LEN]) -> bool {
    // Do not verify checksums if hashing is not enabled.
    // This is necessary so that we don't include blake2b in Wasm
    true
}
