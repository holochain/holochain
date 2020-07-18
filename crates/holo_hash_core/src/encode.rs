use crate::{error::HoloHashError, HashType, HoloHashImpl, PrimitiveHashType};
use std::convert::TryFrom;

impl<P: PrimitiveHashType> TryFrom<&str> for HoloHashImpl<P> {
    type Error = HoloHashError;
    fn try_from(s: &str) -> Result<Self, HoloHashError> {
        let hash_type = P::new();
        Ok(HoloHashImpl::from_raw_bytes(holo_hash_decode(
            hash_type.get_prefix(),
            s,
        )?))
    }
}

impl<P: PrimitiveHashType> TryFrom<&String> for HoloHashImpl<P> {
    type Error = HoloHashError;
    fn try_from(s: &String) -> Result<Self, HoloHashError> {
        Self::try_from(s as &str)
    }
}

impl<P: PrimitiveHashType> TryFrom<String> for HoloHashImpl<P> {
    type Error = HoloHashError;
    fn try_from(s: String) -> Result<Self, HoloHashError> {
        Self::try_from(&s)
    }
}

impl<T: HashType> std::fmt::Display for HoloHashImpl<T> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        let prefix = self.hash_type().get_prefix();
        write!(f, "{}", holo_hash_encode(prefix, self.get_raw()))
    }
}

/// internal REPR for holo hash
pub fn holo_hash_encode(prefix: &[u8], data: &[u8]) -> String {
    format!(
        "u{}{}",
        base64::encode_config(prefix, base64::URL_SAFE_NO_PAD),
        base64::encode_config(data, base64::URL_SAFE_NO_PAD),
    )
}

/// internal PARSE for holo hash REPR
pub fn holo_hash_decode(prefix: &[u8], s: &str) -> Result<Vec<u8>, HoloHashError> {
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    let s = match base64::decode_config(&s[1..], base64::URL_SAFE_NO_PAD) {
        Err(_) => return Err(HoloHashError::BadBase64),
        Ok(s) => s,
    };
    if s.len() != 39 {
        return Err(HoloHashError::BadSize);
    }
    if &s[..3] != prefix {
        return Err(HoloHashError::BadPrefix);
    }
    let s = &s[3..];
    let loc_bytes = holo_dht_location_bytes(&s[..32]);
    let loc_bytes: &[u8] = &loc_bytes;
    if loc_bytes != &s[32..] {
        return Err(HoloHashError::BadChecksum);
    }
    Ok(s.to_vec())
}

/// internal compute the holo dht location u32
pub fn holo_dht_location_bytes(data: &[u8]) -> Vec<u8> {
    // Assert the data size is relatively small so we are
    // comfortable executing this synchronously / blocking tokio thread.
    assert_eq!(32, data.len(), "only 32 byte hashes supported");

    let hash = blake2b_128(data);
    let mut out = vec![hash[0], hash[1], hash[2], hash[3]];
    for i in (4..16).step_by(4) {
        out[0] ^= hash[i];
        out[1] ^= hash[i + 1];
        out[2] ^= hash[i + 2];
        out[3] ^= hash[i + 3];
    }
    out
}

/// internal compute a 32 byte blake2b hash
pub fn blake2b_256(data: &[u8]) -> Vec<u8> {
    let hash = blake2b_simd::Params::new().hash_length(32).hash(data);
    hash.as_bytes().to_vec()
}

/// internal compute a 16 byte blake2b hash
pub fn blake2b_128(data: &[u8]) -> Vec<u8> {
    let hash = blake2b_simd::Params::new().hash_length(16).hash(data);
    hash.as_bytes().to_vec()
}
