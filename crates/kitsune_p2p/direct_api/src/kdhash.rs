//! kdirect kdhash type

use crate::*;

// multihash-like prefixes
//kDAk 6160 <Buffer 90 30 24> <-- using this for KdHash
//kDEk 6288 <Buffer 90 31 24>
//kDIk 6416 <Buffer 90 32 24>
//kDMk 6544 <Buffer 90 33 24>
//kDQk 6672 <Buffer 90 34 24>
//kDUk 6800 <Buffer 90 35 24>
//kDYk 6928 <Buffer 90 36 24>
//kDck 7056 <Buffer 90 37 24>
//kDgk 7184 <Buffer 90 38 24>
//kDkk 7312 <Buffer 90 39 24>
//kDok 7440 <Buffer 90 3a 24>
//kDsk 7568 <Buffer 90 3b 24>
//kDwk 7696 <Buffer 90 3c 24>
//kD0k 7824 <Buffer 90 3d 24>
//kD4k 7952 <Buffer 90 3e 24>
//kD8k 8080 <Buffer 90 3f 24>

const PREFIX: &[u8; 3] = &[0x90, 0x30, 0x24];

/// Kitsune Direct Hash Type
#[derive(Clone)]
pub struct KdHash(pub Arc<(String, [u8; 39])>);

impl serde::Serialize for KdHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0 .0)
    }
}

impl<'de> serde::Deserialize<'de> for KdHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        KdHash::from_str_slice(&String::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

impl std::cmp::PartialEq for KdHash {
    fn eq(&self, other: &Self) -> bool {
        self.0 .0.eq(&other.0 .0)
    }
}

impl std::cmp::Eq for KdHash {}

impl std::cmp::PartialOrd for KdHash {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0 .0.partial_cmp(&other.0 .0)
    }
}

impl std::cmp::Ord for KdHash {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0 .0.cmp(&other.0 .0)
    }
}

impl std::hash::Hash for KdHash {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0 .0.hash(state);
    }
}

impl AsRef<str> for KdHash {
    fn as_ref(&self) -> &str {
        &self.0 .0
    }
}

impl AsRef<[u8]> for KdHash {
    fn as_ref(&self) -> &[u8] {
        &self.0 .1
    }
}

impl AsRef<[u8; 39]> for KdHash {
    fn as_ref(&self) -> &[u8; 39] {
        &self.0 .1
    }
}

impl std::fmt::Debug for KdHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("KdHash").field(&self.0 .0).finish()
    }
}

impl std::fmt::Display for KdHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0 .0.fmt(f)
    }
}

impl From<[u8; 39]> for KdHash {
    fn from(b: [u8; 39]) -> Self {
        Self::from_bytes(b)
    }
}

impl From<[u8; 36]> for KdHash {
    fn from(b: [u8; 36]) -> Self {
        let mut n = [0; 39];
        n[0..3].copy_from_slice(PREFIX);
        n[3..].copy_from_slice(&b);
        n.into()
    }
}

impl KdHash {
    /// Construct a KdHash from raw bytes
    pub fn from_bytes(b: [u8; 39]) -> Self {
        let o = base64::encode_config(b, base64::URL_SAFE_NO_PAD);
        Self(Arc::new((format!("u{}", o), b)))
    }

    /// Construct a KdHash from a &str
    pub fn from_str_slice(b: &str) -> KdResult<Self> {
        let vec = base64::decode_config(&b.as_bytes()[1..], base64::URL_SAFE_NO_PAD)
            .map_err(KdError::other)?;
        if vec.len() != 39 {
            return Err(format!("invalid byte count: {}", vec.len()).into());
        }
        let mut h = [0_u8; 39];
        h.copy_from_slice(&vec[0..39]);

        Ok(Self(Arc::new((b.to_string(), h))))
    }

    /// Get the core 32 bytes of this KdHash,
    /// not including prefix or loc bytes.
    /// For the full 39 bytes, use `as_ref::<[u8]>()`
    pub fn as_core_bytes(&self) -> &[u8; 32] {
        arrayref::array_ref![&self.0 .1, 3, 32]
    }

    /// Get the loc u32 of this KdHash
    pub fn as_loc(&self) -> u32 {
        u32::from_le_bytes(*arrayref::array_ref![&self.0 .1, 35, 4])
    }
}
