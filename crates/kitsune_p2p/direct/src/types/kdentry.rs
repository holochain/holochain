//! kdirect kdentry type

use crate::*;
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
    /// and is what should be used to verify the signature.
    pub wire: Box<[u8]>,

    /// the more human readable encoding of this entry.
    /// this should be stored in databases / sent over websocket connections.
    pub db: String,

    /// the decoded content of this entry, use this for logic.
    pub decoded: KdEntryData,

    /// the hash of this entry, this should be a direct hash
    /// of the wire bytes (not including the signature).
    pub hash: KdHash,
}

fn db_from_wire(wire: &[u8]) -> String {
    let wire_len = wire.len();
    let enc_len = u64::from_le_bytes(*arrayref::array_ref![wire, 0, 8]) as usize;
    let enc = String::from_utf8_lossy(&wire[8..8 + enc_len]);
    let bin = &wire[8 + enc_len..wire_len - 64];
    let bin = base64::encode(bin);
    let sig = &wire[wire_len - 64..wire_len];
    let sig = base64::encode(sig);
    format!("[\n{}\n,\"{}\",\"{}\"]\n", enc, bin, sig)
}

fn wire_from_db(db: &str) -> KitsuneResult<Box<[u8]>> {
    let (_, bin, sig): (serde_json::Value, String, String) =
        serde_json::from_str(db).map_err(KitsuneError::other)?;
    let bin = base64::decode(&bin).map_err(KitsuneError::other)?;
    let sig = base64::decode(&sig).map_err(KitsuneError::other)?;

    let db = db.as_bytes();
    let mut idx = db.len() - 2;
    while idx > 0 {
        if db[idx] == b'\n' {
            break;
        }
        idx -= 1;
    }
    let enc = &db[2..idx];

    let mut wire = Vec::with_capacity(
        8 // len
        + enc.len() // encoded
        + bin.len() // binary
        + 64, // sig
    );

    let enc_len = (enc.len() as u64).to_le_bytes();
    wire.extend_from_slice(&enc_len[..]);
    wire.extend_from_slice(&enc);
    wire.extend_from_slice(&bin);
    wire.extend_from_slice(&sig);

    Ok(wire.into_boxed_slice())
}

impl std::fmt::Debug for KdEntryInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("KdEntry").field(&self.db).finish()
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
    async fn from_checked(wire: Box<[u8]>, db: String) -> KitsuneResult<Self> {
        let enc_len = u64::from_le_bytes(*arrayref::array_ref![&wire, 0, 8]) as usize;
        let enc = &wire[8..8 + enc_len];
        let decoded = serde_json::from_slice(enc).map_err(KitsuneError::other)?;
        let wire_len = wire.len();
        // the signature bytes are not included in the hash
        let hash = KdHash::from_data(&wire[0..wire_len - 64]).await?;
        let entry = Self(Arc::new(KdEntryInner {
            wire,
            db,
            decoded,
            hash,
        }));
        entry.verify_signature().await?;
        Ok(entry)
    }

    /// Build out a full, checked entry from wire encoding
    pub async fn from_wire_checked(wire: Box<[u8]>) -> KitsuneResult<Self> {
        let db = db_from_wire(&wire);
        Self::from_checked(wire, db).await
    }

    /// Build out a full, checked entry from db encoding
    pub async fn from_db_checked(db: String) -> KitsuneResult<Self> {
        let wire = wire_from_db(&db)?;
        Self::from_checked(wire, db).await
    }

    /// Access the data of this entry
    pub fn as_data(&self) -> &KdEntryData {
        self.0.as_data()
    }

    /// Get the hash of this entry
    pub fn as_hash(&self) -> &KdHash {
        &self.0.hash
    }

    /// Get the binary data associated with this entry
    pub fn as_binary(&self) -> &[u8] {
        let wire_len = self.0.wire.len();
        let enc_len = u64::from_le_bytes(*arrayref::array_ref![&self.0.wire, 0, 8]) as usize;
        &self.0.wire[8 + enc_len..wire_len - 64]
    }

    /// Get the signature bytes associated with this entry
    pub fn as_signature(&self) -> &[u8; 64] {
        let len = self.0.wire.len();
        arrayref::array_ref![&self.0.wire, len - 64, 64]
    }

    /// Get the wire encoding for this entry
    pub fn as_wire(&self) -> &[u8] {
        &self.0.wire
    }

    /// Get the db encoding for this entry
    pub fn as_db(&self) -> &str {
        &self.0.db
    }

    /// Returns `Ok(())` if the signature is valid for the internal data
    pub async fn verify_signature(&self) -> KitsuneResult<()> {
        let wire_len = self.0.wire.len();
        let signature = Arc::new(*self.as_signature());
        let data = &self.0.wire[0..wire_len - 64];
        let data = Buffer::from_ref(data);
        if self
            .as_data()
            .author
            .verify_signature(data, signature)
            .await
        {
            Ok(())
        } else {
            Err("invalid signature".into())
        }
    }

    /// Sign entry data into a full KdEntry instance
    pub fn sign(
        persist: &KdPersist,
        decoded: KdEntryData,
    ) -> impl Future<Output = KitsuneResult<Self>> + 'static + Send {
        Self::sign_with_binary(persist, decoded, &[])
    }

    /// Sign entry data into a full KdEntry instance with additional binary data
    pub fn sign_with_binary(
        persist: &KdPersist,
        decoded: KdEntryData,
        binary: &[u8],
    ) -> impl Future<Output = KitsuneResult<Self>> + 'static + Send {
        let wire = (|| {
            let encoded = serde_json::to_string_pretty(&decoded)
                .map_err(KitsuneError::other)?
                .into_bytes();
            let encoded_len = (encoded.len() as u64).to_le_bytes();
            let mut wire = Vec::with_capacity(
                8 // len
                + encoded.len() // encoded
                + binary.len() // binary
                + 64, // sig
            );
            wire.extend_from_slice(&encoded_len[..]);
            wire.extend_from_slice(&encoded);
            wire.extend_from_slice(binary);
            KitsuneResult::Ok(wire)
        })();

        let persist = persist.clone();
        async move {
            let mut wire = wire?;

            // these two ops don't include the signature bytes
            // for obvious reasons
            let signature = persist.sign(decoded.author.clone(), &wire).await?;
            let hash = KdHash::from_data(&wire).await?;

            wire.extend_from_slice(&signature[..]);

            let wire = wire.into_boxed_slice();

            let db = db_from_wire(&wire);

            Ok(Self(Arc::new(KdEntryInner {
                wire,
                db,
                decoded,
                hash,
            })))
        }
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
        let wire = entry.as_wire();
        println!("wire: {}", String::from_utf8_lossy(wire));
        let e2 = KdEntry::from_wire_checked(wire.to_vec().into_boxed_slice())
            .await
            .unwrap();
        assert_eq!(e2, entry);
        assert_eq!(&[0, 1, 2, 3][..], e2.as_binary());
        let db = entry.as_db();
        println!("db: {}", db);
        let e3 = KdEntry::from_db_checked(db.to_string()).await.unwrap();
        assert_eq!(e3, entry);
        assert_eq!(&[0, 1, 2, 3][..], e3.as_binary());
    }
}
