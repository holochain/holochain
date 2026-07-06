use crate::assert_length;
use crate::error::HoloHashError;
use crate::HashType;
use crate::HoloHash;
use crate::PrimitiveHashType;
use crate::HOLO_HASH_CORE_LEN;
use crate::HOLO_HASH_FULL_LEN;
use crate::HOLO_HASH_PREFIX_LEN;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use std::convert::TryFrom;
use std::convert::TryInto;
use crate::location::holo_dht_location_bytes;

impl<P: PrimitiveHashType> TryFrom<&str> for HoloHash<P> {
    type Error = HoloHashError;
    fn try_from(s: &str) -> Result<Self, HoloHashError> {
        let hash_type = P::new();
        HoloHash::try_from_raw_39(holo_hash_decode(hash_type.get_prefix(), s)?)
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
    format!("u{}", URL_SAFE_NO_PAD.encode(data),)
}

/// internal PARSE for holo hash REPR
pub fn holo_hash_decode_unchecked(s: &str) -> Result<Vec<u8>, HoloHashError> {
    // 1 /* u */ + ((3 /* prefix */ + 32 /* hash */ + 4 /* loc */ ) * 4 / 3) == 53
    if s.len() != 53 {
        return Err(HoloHashError::BadSize);
    }
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    let b = match URL_SAFE_NO_PAD.decode(&s[1..]) {
        Err(_) => return Err(HoloHashError::BadBase64),
        Ok(s) => s,
    };
    if b.len() != HOLO_HASH_FULL_LEN {
        return Err(HoloHashError::BadSize);
    }
    let loc_bytes = holo_dht_location_bytes(
        &b[HOLO_HASH_PREFIX_LEN..HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN],
    );
    let loc_bytes: &[u8] = &loc_bytes;
    if loc_bytes != &b[HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN..] {
        return Err(HoloHashError::BadChecksum(s.to_string()));
    }
    assert_length!(HOLO_HASH_FULL_LEN, &b);
    Ok(b.to_vec())
}

/// internal PARSE for holo hash REPR
pub fn holo_hash_decode(prefix: &[u8], s: &str) -> Result<Vec<u8>, HoloHashError> {
    if &s[..1] != "u" {
        return Err(HoloHashError::NoU);
    }
    let b = match URL_SAFE_NO_PAD.decode(&s[1..]) {
        Err(_) => return Err(HoloHashError::BadBase64),
        Ok(s) => s,
    };
    if b.len() != HOLO_HASH_FULL_LEN {
        return Err(HoloHashError::BadSize);
    }
    let actual_prefix: [u8; HOLO_HASH_PREFIX_LEN] = b[..HOLO_HASH_PREFIX_LEN].try_into().unwrap();
    if actual_prefix != prefix {
        return Err(HoloHashError::BadPrefix(
            format!("{prefix:?}"),
            actual_prefix,
        ));
    }
    let loc_bytes = holo_dht_location_bytes(
        &b[HOLO_HASH_PREFIX_LEN..HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN],
    );
    let loc_bytes: &[u8] = &loc_bytes;
    if loc_bytes != &b[HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN..] {
        return Err(HoloHashError::BadChecksum(s.to_string()));
    }
    assert_length!(HOLO_HASH_FULL_LEN, &b);
    Ok(b.to_vec())
}
