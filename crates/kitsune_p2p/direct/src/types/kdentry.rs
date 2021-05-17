//! kdirect kdentry type

use crate::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use types::kdhash::KdHash;
use types::persist::KdPersist;

/// Inner content data of a KdEntry
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct KdEntryData {
    /// type indicator hint for this entry
    #[serde(rename = "t")]
    pub type_hint: String,

    /// parent hash reference for this entry
    #[serde(rename = "p")]
    pub parent: KdHash,

    /// the hash (pubkey) of the author of this entry
    #[serde(rename = "a")]
    pub author: KdHash,

    /// indicates if this entry should be sharded
    #[serde(rename = "s")]
    pub should_shard: bool,

    /// interval after which this entry should be re-verified
    /// TODO - FIXME - remove this, should be returned by verify fn
    #[serde(rename = "r")]
    pub reverify_interval_s: u32,

    /// process to follow for verifying children to this entry
    #[serde(rename = "v")]
    pub verify: String,

    /// type-specific data content of this entry
    #[serde(rename = "d")]
    pub data: serde_json::Value,
}

impl KdEntryData {
    /// Translate the data section of this entry into a compatible
    /// rust structure.
    pub fn translate_data<D>(&self) -> KitsuneResult<D>
    where
        D: serde::de::DeserializeOwned,
    {
        serde_json::from_value(self.data.clone()).map_err(KitsuneError::other)
    }
}

/// Inner signature, hash, encoded, and decoded data of a KdEntry
pub struct KdEntryInner {
    /// the encoded bytes of this entry.
    /// this (and the signature) should be sent over the network,
    /// and is what should be used to verify signature.
    pub encoded: String,

    /// the additional binary data associated with this entry.
    pub binary: Box<[u8]>,

    /// the decoded content of this entry, use this for logic.
    pub decoded: KdEntryData,

    /// the hash of this entry, this should be a direct hash
    /// of the encoded bytes.
    pub hash: KdHash,

    /// the signature of the encoded bytes
    pub signature: Arc<[u8; 64]>,
}

impl std::fmt::Debug for KdEntryInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("KdEntry")
            .field(&self.hash)
            .field(&self.decoded)
            .finish()
    }
}

impl std::ops::Deref for KdEntryInner {
    type Target = KdEntryData;

    fn deref(&self) -> &Self::Target {
        self.as_data()
    }
}

impl KdEntryInner {
    /// Access the data of this entry
    pub fn as_data(&self) -> &KdEntryData {
        &self.decoded
    }
}

/// A KdEntry is a mono-type representing all data in kitsune direct.
#[derive(Debug, Clone)]
pub struct KdEntry(pub Arc<KdEntryInner>);

impl PartialEq for KdEntry {
    fn eq(&self, oth: &Self) -> bool {
        self.0.hash.eq(&oth.0.hash)
    }
}

impl Eq for KdEntry {}

impl std::hash::Hash for KdEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash.hash(state);
    }
}

impl std::ops::Deref for KdEntry {
    type Target = KdEntryData;

    fn deref(&self) -> &Self::Target {
        self.as_data()
    }
}

impl KdEntry {
    /// Access the data of this entry
    pub fn as_data(&self) -> &KdEntryData {
        self.0.as_data()
    }

    /// Get the hash of this entry
    pub fn hash(&self) -> &KdHash {
        &self.0.hash
    }

    /// Get the binary data associated with this entry
    pub fn binary(&self) -> &[u8] {
        &self.0.binary
    }

    /// Sign entry data into a full KdEntry instance
    pub async fn sign(persist: &KdPersist, decoded: KdEntryData) -> KitsuneResult<Self> {
        Self::sign_with_binary(persist, decoded, &[]).await
    }

    /// Sign entry data into a full KdEntry instance with additional binary data
    pub async fn sign_with_binary(
        persist: &KdPersist,
        decoded: KdEntryData,
        binary: &[u8],
    ) -> KitsuneResult<Self> {
        let encoded = serde_json::to_string(&decoded).map_err(KitsuneError::other)?;
        let binary = binary.to_vec().into_boxed_slice();
        let hash = KdHash::from_data(encoded.as_bytes()).await?;
        let signature = persist
            .sign(decoded.author.clone(), encoded.as_bytes())
            .await?;
        Ok(Self(Arc::new(KdEntryInner {
            encoded,
            binary,
            decoded,
            hash,
            signature,
        })))
    }

    /// Encode this entry for storage or transmition
    pub fn encode(&self) -> PoolBuf {
        let sig_b64 = base64::encode_config(&self.0.signature[..], base64::URL_SAFE_NO_PAD);
        // the binary is separate specifically so we DON'T have to b64 encode it
        // eventually, it will just be tagged onto the end
        // but for now this is easier to debug
        let bin_b64 = base64::encode_config(&self.0.binary[..], base64::URL_SAFE_NO_PAD);
        let full = serde_json::to_string(&(sig_b64, &self.0.encoded, bin_b64)).unwrap();
        let full = full.as_bytes();
        let mut out = PoolBuf::new();
        out.extend_from_slice(full);
        out
    }

    /// Decode and check signature on an encoded signature + entry
    pub async fn decode_checked(entry: &[u8]) -> KitsuneResult<Self> {
        let (sig, encoded, bin): (String, String, String) =
            serde_json::from_slice(entry).map_err(KitsuneError::other)?;
        let mut signature = [0; 64];
        let sig =
            base64::decode_config(sig, base64::URL_SAFE_NO_PAD).map_err(KitsuneError::other)?;
        let binary = base64::decode_config(bin, base64::URL_SAFE_NO_PAD)
            .map_err(KitsuneError::other)?
            .into_boxed_slice();
        signature.copy_from_slice(&sig);
        let signature = Arc::new(signature);
        let decoded: KdEntryData = serde_json::from_str(&encoded).map_err(KitsuneError::other)?;
        let data = Buffer::from_ref(encoded.as_bytes());
        if !decoded
            .author
            .verify_signature(data, signature.clone())
            .await
        {
            return Err("invalid signature".into());
        }
        let hash = KdHash::from_data(encoded.as_bytes()).await?;
        Ok(Self(Arc::new(KdEntryInner {
            encoded,
            binary,
            decoded,
            hash,
            signature,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_kdentry_codec() {
        let persist = crate::persist_mem::new_persist_mem();
        let agent = persist.generate_signing_keypair().await.unwrap();
        let binary = [0, 1, 2, 3];

        let edata = KdEntryData {
            type_hint: "s.root".to_string(),
            parent: [0; 39].into(),
            author: agent,
            should_shard: false,
            reverify_interval_s: 60 * 60 * 24,
            verify: "".to_string(),
            data: serde_json::json!({
                "hello": "world",
            }),
        };
        let entry = KdEntry::sign_with_binary(&persist, edata, &binary[..])
            .await
            .unwrap();
        println!("{:?}", &entry);
        let wire = entry.encode();
        println!("wire: {}", String::from_utf8_lossy(&wire));
        let e2 = KdEntry::decode_checked(&wire).await.unwrap();
        assert_eq!(e2, entry);
        assert_eq!(&[0, 1, 2, 3][..], e2.binary());
    }
}
