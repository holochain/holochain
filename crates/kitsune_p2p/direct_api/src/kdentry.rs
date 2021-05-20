//! kdirect kdentry type

use crate::*;

/// Inner content data of a KdEntry
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct KdEntryContent {
    /// system/user type/kind indicator hint for this entry
    ///
    /// all system kinds will begin with 's.'
    /// all user kinds will begin with 'u.'
    /// no other prefixes will be accepted.
    #[serde(rename = "kind")]
    pub kind: String,

    /// parent hash reference for this entry
    #[serde(rename = "parent")]
    pub parent: KdHash,

    /// the hash (pubkey) of the author of this entry
    #[serde(rename = "author")]
    pub author: KdHash,

    /// process to follow for verifying children to this entry
    ///
    /// this logic can also configure strategies for storing / re-verifying
    /// such as:
    ///  - `should_shard` - Default: false
    ///  - `reverify_interval_ms` - Default: u64::MAX
    #[serde(rename = "verify")]
    pub verify: String,

    /// kind-specific data content of this entry
    #[serde(rename = "data")]
    pub data: serde_json::Value,
}

impl KdEntryContent {
    /// encode this data, plus an optional binary byte array
    /// into a `Vec<u8>` suitable for hashing and signing
    /// and eventually integrating into a KdEntrySigned struct.
    pub fn to_data_to_sign(&self, binary: Vec<u8>) -> KdResult<Vec<u8>> {
        let mut out = binary;
        serde_json::to_writer(&mut out, self).map_err(KdError::other)?;
        Ok(out)
    }
}

/// Additional types associated with the KdEntrySigned struct.
pub mod kd_entry {
    use super::*;

    /// Inner binary data of a KdEntry
    pub struct KdEntryBinary(Box<[u8]>);

    impl From<Box<[u8]>> for KdEntryBinary {
        fn from(d: Box<[u8]>) -> Self {
            Self(d)
        }
    }

    impl std::ops::Deref for KdEntryBinary {
        type Target = [u8];

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl AsRef<[u8]> for KdEntryBinary {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl std::borrow::Borrow<[u8]> for KdEntryBinary {
        fn borrow(&self) -> &[u8] {
            &self.0
        }
    }

    impl std::fmt::Debug for KdEntryBinary {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let byte_count = self.0.len();
            f.debug_struct("KdEntryBinary")
                .field("byte_count", &byte_count)
                .finish()
        }
    }

    impl serde::Serialize for KdEntryBinary {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let s = base64::encode(&self.0);
            serializer.serialize_str(&s)
        }
    }

    impl<'de> serde::Deserialize<'de> for KdEntryBinary {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer).map_err(serde::de::Error::custom)?;
            let d = base64::decode(&s).map_err(serde::de::Error::custom)?;
            Ok(Self(d.into_boxed_slice()))
        }
    }

    /// The serialized version of this struct will be used for database
    /// and local websocket storage / transmission of signed KdEntry data.
    /// The `wire_data` portion is all that is needed for wire transmission,
    /// but this full struct is more human parse-able.
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct KdEntrySignedInner {
        /// decoded content data associated with this entry
        #[serde(rename = "content")]
        pub content: KdEntryContent,

        /// the hash of this entry
        #[serde(rename = "hash")]
        pub hash: KdHash,

        /// the byte count (length) of the binary data associated with this entry
        #[serde(rename = "binaryLen")]
        pub binary_len: usize,

        /// the wire encoding for this signed entry
        ///
        /// [binary(var), encoded(var), hash(39), signature(64), binary_len(4)]
        ///
        /// parse this from the end first:
        ///   binary_len - the final 4 bytes are a u32_le, this u32 represents
        ///                the byte count (length) of the 'binary' data
        ///    signature - the previous 64 bytes are the signature data
        ///         hash - the previous 39 bytes are the hash
        ///      encoded - bytes 'binary_len' to just before the hash are the
        ///                encoded content bytes
        ///       binary - bytes 0 to 'binary_len' are the binary data associated
        ///                with this entry
        #[serde(rename = "wireData")]
        pub wire_data: KdEntryBinary,
    }
}

use kd_entry::*;

/// A KdEntry is a mono-type representing all data in kitsune direct.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KdEntrySigned(pub Arc<KdEntrySignedInner>);

impl std::fmt::Display for KdEntrySigned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string_pretty(&self.0).map_err(|_| std::fmt::Error)?;
        f.write_str(&s)?;
        Ok(())
    }
}

impl PartialEq for KdEntrySigned {
    fn eq(&self, oth: &Self) -> bool {
        self.as_hash_ref().eq(oth.as_hash_ref())
    }
}

impl Eq for KdEntrySigned {}

impl std::hash::Hash for KdEntrySigned {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_hash_ref().hash(state);
    }
}

struct Parsed<'lt> {
    pub binary_len: usize,
    pub signature: &'lt [u8],
    pub hash: &'lt [u8],
    pub encoded: &'lt [u8],
    pub binary: &'lt [u8],
    pub data_to_sign: &'lt [u8],
}

/// internal helper to parse the contents of wire_data
fn parse<'a>(wire: &'a [u8]) -> KdResult<Parsed<'a>> {
    const S_BIN_LEN: usize = 4;
    const S_SIG: usize = S_BIN_LEN + 64;
    const S_HASH: usize = S_SIG + 39;

    let wire_len = wire.len();

    if wire_len < S_HASH {
        return Err("invalid wire encoding for KdEntry".into());
    }

    let binary_len = u32::from_le_bytes(*arrayref::array_ref![wire, wire_len - 4, 4]) as usize;

    if wire_len < S_HASH + binary_len {
        return Err("invalid wire encoding for KdEntry".into());
    }

    Ok(Parsed {
        binary_len,
        signature: &wire[wire_len - S_SIG..wire_len - S_BIN_LEN],
        hash: &wire[wire_len - S_HASH..wire_len - S_SIG],
        encoded: &wire[binary_len..wire_len - S_HASH],
        binary: &wire[0..binary_len],
        data_to_sign: &wire[0..wire_len - S_HASH],
    })
}

impl KdEntrySigned {
    /// Parse wire data into a full KdEntrySigned struct.
    /// ! unchecked ! - hash / signature is not verified by this function.
    pub fn from_wire_unchecked(wire: Box<[u8]>) -> KdResult<Self> {
        let parsed = parse(&wire)?;
        let content = serde_json::from_slice(parsed.encoded).map_err(KdError::other)?;
        let hash = *arrayref::array_ref![parsed.hash, 0, 39];
        let hash = KdHash::from_bytes(hash);
        let binary_len = parsed.binary_len;
        drop(parsed);
        let wire_data = wire.into();
        Ok(Self(Arc::new(KdEntrySignedInner {
            content,
            hash,
            binary_len,
            wire_data,
        })))
    }

    /// Construct a full KdEntrySigned from requisit components.
    /// See KdEntryContent::to_data_to_sign for `data_to_sign`.
    /// ! unchecked ! - hash / signature is not verified by this function.
    pub fn from_components_unchecked(
        data_to_sign: Vec<u8>,
        binary_len: usize,
        hash: &[u8; 39],
        signature: &[u8; 64],
    ) -> KdResult<Self> {
        let binary_len = binary_len as u32;
        let binary_len = binary_len.to_le_bytes();
        let mut wire_data = data_to_sign;
        wire_data.reserve(39 + 64 + 4);
        wire_data.extend_from_slice(&hash[..]);
        wire_data.extend_from_slice(&signature[..]);
        wire_data.extend_from_slice(&binary_len[..]);
        Self::from_wire_unchecked(wire_data.into_boxed_slice())
    }

    /// Reconstruct a KdEntrySigned from a `to_string()` str.
    /// ! unchecked ! - hash / signature is not verified by this function.
    pub fn from_str_unchecked(s: &str) -> KdResult<Self> {
        serde_json::from_str(s).map_err(KdError::other)
    }

    /// get the signature data for this signed entry
    pub fn as_signature_ref(&self) -> &[u8; 64] {
        let sig = parse(&self.0.wire_data).unwrap().signature;
        arrayref::array_ref![sig, 0, 64]
    }

    /// get the hash for this signed entry
    pub fn as_hash_ref(&self) -> &[u8; 39] {
        let hash = parse(&self.0.wire_data).unwrap().hash;
        arrayref::array_ref![hash, 0, 39]
    }

    /// Get the encoded content associated with this signed entry
    pub fn as_encoded_ref(&self) -> &[u8] {
        parse(&self.0.wire_data).unwrap().encoded
    }

    /// Get the binary data associated with this signed entry
    pub fn as_binary_ref(&self) -> &[u8] {
        parse(&self.0.wire_data).unwrap().binary
    }

    /// get the binary data that is used for hashing/signature
    pub fn as_data_to_sign_ref(&self) -> &[u8] {
        parse(&self.0.wire_data).unwrap().data_to_sign
    }

    /// get the full binary wire data repr of this signed entry
    pub fn as_wire_data_ref(&self) -> &[u8] {
        &self.0.wire_data
    }

    /// get the KdHash for this signed entry
    pub fn hash(&self) -> &KdHash {
        &self.0.hash
    }

    /// get the lengeth of the binary data associated with this signed entry
    pub fn binary_len(&self) -> usize {
        self.0.binary_len
    }

    /// get the content kind of this signed entry
    pub fn kind(&self) -> &str {
        &self.0.content.kind
    }

    /// get the content parent of this signed entry
    pub fn parent(&self) -> &KdHash {
        &self.0.content.parent
    }

    /// get the content author of this signed entry
    pub fn author(&self) -> &KdHash {
        &self.0.content.author
    }

    /// get the content verify logic of this signed entry
    pub fn verify(&self) -> &str {
        &self.0.content.verify
    }

    /// get the raw content data associated with this signed entry
    pub fn raw_data(&self) -> &serde_json::Value {
        &self.0.content.data
    }

    /// translate the content data of this entry into a compatible rust struct
    pub fn translate_data<D>(&self) -> KdResult<D>
    where
        D: serde::de::DeserializeOwned,
    {
        serde_json::from_value(self.0.content.data.clone()).map_err(KdError::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kd_sys_kind::*;

    #[test]
    fn kdentry_encode_decode() {
        let binary = vec![1, 2, 3, 4];
        let content = KdEntryContent {
            kind: "s.app".to_string(),
            parent: [0; 36].into(),
            author: [1; 36].into(),
            verify: "".to_string(),
            data: KdSysKindApp {
                name: "test".to_string(),
            }
            .to_json()
            .unwrap(),
        };
        let binary_len = binary.len();
        let content = content.to_data_to_sign(binary).unwrap();
        let e1 = KdEntrySigned::from_components_unchecked(content, binary_len, &[2; 39], &[3; 64])
            .unwrap();
        println!("{:#?}", e1);
        let s = e1.to_string();
        println!("{}", s);
        let w = e1.as_wire_data_ref().to_vec().into_boxed_slice();
        let e2 = KdEntrySigned::from_wire_unchecked(w).unwrap();
        println!("{:#?}", e2);
        assert_eq!(e1, e2);
        let e3 = KdEntrySigned::from_str_unchecked(&s).unwrap();
        println!("{:#?}", e3);
        assert_eq!(e1, e3);
    }
}
