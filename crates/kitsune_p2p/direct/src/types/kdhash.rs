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
        Ok(KdHash::from_str_slice(&String::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)?)
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

impl KdHash {
    /// Construct a KdHash from raw bytes
    pub fn from_bytes(b: [u8; 39]) -> Self {
        let o = base64::encode_config(b, base64::URL_SAFE_NO_PAD);
        Self(Arc::new((format!("u{}", o), b)))
    }

    /// Construct a KdHash from a &str
    pub fn from_str_slice(b: &str) -> KitsuneResult<Self> {
        let vec = base64::decode_config(&b.as_bytes()[1..], base64::URL_SAFE_NO_PAD)
            .map_err(KitsuneError::other)?;
        let mut h = [0_u8; 39];
        h.copy_from_slice(&vec[0..39]);

        Ok(Self(Arc::new((b.to_string(), h))))
    }

    /// Get the true hash portion (32 bytes) of this KdHash
    pub fn as_hash(&self) -> &[u8] {
        arrayref::array_ref![&self.0 .1, 3, 32]
    }

    /// Get the loc u32 of this KdHash
    pub fn as_loc(&self) -> u32 {
        u32::from_le_bytes(*arrayref::array_ref![&self.0 .1, 35, 4])
    }

    /// Get the true hash portion (32 bytes) of this KdHash as a sodoken Buffer
    pub fn as_buffer(&self) -> Buffer {
        Buffer::from_ref(self.as_hash())
    }

    /// Treating this hash as a sodoken pubkey,
    /// verify the given data / signature
    pub async fn verify_signature(&self, data: sodoken::Buffer, signature: Arc<[u8; 64]>) -> bool {
        match async {
            let pk = self.as_buffer();
            let sig = Buffer::from_ref(&signature[..]);
            KitsuneResult::Ok(
                sodoken::sign::sign_verify_detached(&sig, &data, &pk)
                    .await
                    .map_err(KitsuneError::other)?,
            )
        }
        .await
        {
            Ok(r) => r,
            Err(_) => false,
        }
    }

    /// Generate a KdHash from data
    pub async fn from_data(data: &[u8]) -> KitsuneResult<Self> {
        let r = Buffer::from_ref(data);

        let mut hash = Buffer::new(32);
        sodoken::hash::generichash(&mut hash, &r, None)
            .await
            .map_err(KitsuneError::other)?;
        let hash = hash.read_lock().to_vec();

        // we can use the coerce function now that we have a real hash
        // for the data... even though it's not a pubkey--DRY
        Self::from_coerced_pubkey(&hash).await
    }

    /// Coerce 32 bytes of signing pubkey data into a KdHash
    pub async fn from_coerced_pubkey(data: &[u8]) -> KitsuneResult<Self> {
        assert_eq!(32, data.len());

        let loc = loc_hash(data).await?;

        let mut out = [0; 39];
        out[0] = 0x90;
        out[1] = 0x30;
        out[2] = 0x24;
        out[3..35].copy_from_slice(data);
        out[35..].copy_from_slice(&loc);

        Ok(Self::from_bytes(out))
    }
}

async fn loc_hash(d: &[u8]) -> KitsuneResult<[u8; 4]> {
    let mut out = [0; 4];

    let d: Buffer = d.to_vec().into();
    let mut hash = Buffer::new(16);
    sodoken::hash::generichash(&mut hash, &d, None)
        .await
        .map_err(KitsuneError::other)?;

    let hash = hash.read_lock();
    out[0] = hash[0];
    out[1] = hash[1];
    out[2] = hash[2];
    out[3] = hash[3];
    for i in (4..16).step_by(4) {
        out[0] ^= hash[i];
        out[1] ^= hash[i + 1];
        out[2] ^= hash[i + 2];
        out[3] ^= hash[i + 3];
    }

    Ok(out)
}
