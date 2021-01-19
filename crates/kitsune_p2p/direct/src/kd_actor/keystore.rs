use super::*;
use sodoken::*;

pub(crate) trait AsKeystore: 'static + Send + Sync {
    /// Generate a new signing agent.
    fn generate_sign_agent(&self) -> ghost_actor::GhostFuture<KdHash, KdError>;

    /// Sign with secret key associated with pub key `pk`.
    fn sign(
        &self,
        pk: KdHash,
        data: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<Arc<[u8; 64]>, KdError>;

    ghost_actor::ghost_box_trait_fns!(AsKeystore);
}
ghost_actor::ghost_box_trait!(AsKeystore);

pub(crate) struct Keystore(Box<dyn AsKeystore>);
ghost_actor::ghost_box_new_type!(Keystore);

impl Keystore {
    /// Generate a new signing agent.
    pub fn generate_sign_agent(&self) -> ghost_actor::GhostFuture<KdHash, KdError> {
        AsKeystore::generate_sign_agent(&*self.0)
    }

    /// Sign with secret key associated with pub key `pk`.
    pub fn sign(
        &self,
        pk: KdHash,
        data: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<Arc<[u8; 64]>, KdError> {
        AsKeystore::sign(&*self.0, pk, data)
    }
}

pub(crate) fn spawn_keystore(persist: Persist) -> Keystore {
    KdKeystore::new(persist)
}

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

async fn loc_hash(d: &[u8]) -> KdResult<[u8; 4]> {
    let mut out = [0; 4];

    let d: Buffer = d.to_vec().into();
    let mut hash = Buffer::new(16);
    hash::generichash(&mut hash, &d, None).await?;

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

/// Hash type used by Kd
#[derive(Clone)]
pub struct KdHash(Arc<(String, once_cell::sync::OnceCell<[u8; 39]>)>);

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
        Ok(KdHash::from_str_unchecked(&String::deserialize(
            deserializer,
        )?))
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

impl KdHash {
    /// hash the given data to produce a valid KdHash.
    pub async fn from_data<R: AsRef<[u8]>>(r: R) -> KdResult<Self> {
        let r = r.as_ref();
        let r = Buffer::from_ref(r);

        let mut hash = Buffer::new(32);
        hash::generichash(&mut hash, &r, None).await?;

        let r: Box<[u8]> = (&*hash.read_lock()).into();
        Self::from_bytes(r).await
    }

    /// create a validated KdHash from an AsRef<str> item.
    pub async fn from_str<R: AsRef<str>>(r: R) -> KdResult<Self> {
        let r = r.as_ref();
        let mut iter = r.bytes();
        if iter.next() != Some(117) {
            return Err("KdHash strings must begin with 'u'".into());
        }
        let d = base64::decode_config(&iter.collect::<Vec<_>>(), base64::URL_SAFE_NO_PAD)
            .map_err(KdError::other)?;
        Self::from_bytes(d.into()).await
    }

    /// create a validated KdHash from raw sodoken buffer.
    pub async fn from_sodoken(r: &Buffer) -> KdResult<Self> {
        let r: Box<[u8]> = (&*r.read_lock()).into();
        Self::from_bytes(r).await
    }

    /// create a validated KdHash from raw bytes.
    pub async fn from_bytes(r: Box<[u8]>) -> KdResult<Self> {
        let r = &r;
        let mut buf = [0_u8; 39];
        match r.len() {
            32 => {
                buf[0] = 0x90;
                buf[1] = 0x30;
                buf[2] = 0x24;
                buf[3..35].copy_from_slice(r);
                let loc = loc_hash(&buf[3..35]).await?;
                buf[35..].copy_from_slice(&loc);
            }
            36 => {
                buf[0] = 0x90;
                buf[1] = 0x30;
                buf[2] = 0x24;
                buf[3..].copy_from_slice(r);
                let loc = loc_hash(&buf[3..35]).await?;
                if buf[35..] != loc[..] {
                    return Err("invalid loc bytes".into());
                }
            }
            39 => {
                if r[0] != 0x90 || r[1] != 0x30 || r[2] != 0x24 {
                    return Err("invalid hash prefix".into());
                }
                buf[..].copy_from_slice(r);
                let loc = loc_hash(&buf[3..35]).await?;
                if buf[35..] != loc[..] {
                    return Err("invalid loc bytes".into());
                }
            }
            _ => return Err(format!("invalid byte length: {}", r.len()).into()),
        }
        Ok(Self(Arc::new((
            format!(
                "u{}",
                base64::encode_config(&buf[..], base64::URL_SAFE_NO_PAD)
            ),
            once_cell::sync::OnceCell::new(),
        ))))
    }

    /// create an UNVALIDATED KdHash from raw str.
    /// This is really only meant to be used internally from
    /// sources that were previously validated.
    pub fn from_str_unchecked<R: AsRef<str>>(s: R) -> Self {
        Self(Arc::new((
            s.as_ref().to_string(),
            once_cell::sync::OnceCell::new(),
        )))
    }

    /// create an UNVALIDATED KdHash from raw bytes.
    /// This is really only meant to be used internally from
    /// sources that were previously validated.
    pub fn from_36_bytes_unchecked(r: &[u8; 36]) -> Self {
        let s = format!(
            "u{}{}",
            base64::encode_config(&[0x90, 0x30, 0x24], base64::URL_SAFE_NO_PAD),
            base64::encode_config(&r[..], base64::URL_SAFE_NO_PAD),
        );
        Self(Arc::new((s, once_cell::sync::OnceCell::new())))
    }

    /// get the raw bytes underlying this hash instance
    pub fn get_raw_bytes(&self) -> &[u8; 39] {
        self.0 .1.get_or_init(|| {
            let mut out = [0; 39];
            out.copy_from_slice(
                &base64::decode_config(
                    self.0 .0.bytes().skip(1).collect::<Vec<_>>(),
                    base64::URL_SAFE_NO_PAD,
                )
                .unwrap()[..39],
            );
            out
        })
    }

    /// get the 32 true hash bytes ([3..35])
    pub fn get_hash_bytes(&self) -> &[u8; 32] {
        arrayref::array_ref![self.get_raw_bytes(), 3, 32]
    }

    /// get the 32 true hash bytes as a sodoken buffer ([3..35])
    pub fn get_sodoken(&self) -> Buffer {
        let out = Buffer::new(32);
        out.write_lock().copy_from_slice(self.get_hash_bytes());
        out
    }

    /// get the 4 trailing location bytes ([35..]) as a u32
    pub fn get_loc(&self) -> u32 {
        let bytes = self.get_raw_bytes();
        (bytes[35] as u32)
            + ((bytes[36] as u32) << 8)
            + ((bytes[37] as u32) << 16)
            + ((bytes[38] as u32) << 24)
    }

    /// assuming this KdHash is a pub_key - validate the
    /// given signature applies to given data.
    pub async fn verify_signature(&self, data: sodoken::Buffer, signature: Arc<[u8; 64]>) -> bool {
        match async {
            let pk = self.get_sodoken();
            let sig = Buffer::from_ref(&signature[..]);
            KdResult::Ok(sign::sign_verify_detached(&sig, &data, &pk).await?)
        }
        .await
        {
            Ok(r) => r,
            Err(_) => false,
        }
    }
}

macro_rules! hf {
    ($k:ident) => {
        impl From<&$k> for KdHash {
            fn from(o: &$k) -> Self {
                let bytes = arrayref::array_ref![&o.0, 0, 36];
                Self::from_36_bytes_unchecked(bytes)
            }
        }

        impl From<Arc<$k>> for KdHash {
            fn from(o: Arc<$k>) -> Self {
                let bytes = arrayref::array_ref![&o, 0, 36];
                Self::from_36_bytes_unchecked(bytes)
            }
        }

        impl From<KdHash> for $k {
            fn from(o: KdHash) -> Self {
                Self(o.get_raw_bytes()[3..].to_vec())
            }
        }

        impl From<KdHash> for Arc<$k> {
            fn from(o: KdHash) -> Self {
                $k::from(o).into()
            }
        }
    };
}

hf!(KitsuneSpace);
hf!(KitsuneAgent);
hf!(KitsuneOpHash);

struct KdKeystoreInner {
    persist: Persist,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct KdKeystore(ghost_actor::GhostActor<KdKeystoreInner>);

impl KdKeystore {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(persist: Persist) -> Keystore {
        let (actor, driver) = ghost_actor::GhostActor::new(KdKeystoreInner { persist });
        tokio::task::spawn(driver);
        Keystore(Box::new(Self(actor)))
    }
}

impl AsKeystore for KdKeystore {
    ghost_actor::ghost_box_trait_impl_fns!(AsKeystore);

    fn generate_sign_agent(&self) -> ghost_actor::GhostFuture<KdHash, KdError> {
        let actor = self.0.clone();
        ghost_actor::resp(async move {
            let mut pk = Buffer::new(sign::SIGN_PUBLICKEYBYTES);
            let mut sk = Buffer::new_memlocked(sign::SIGN_SECRETKEYBYTES)?;

            sign::sign_keypair(&mut pk, &mut sk).await?;

            let pk = KdHash::from_sodoken(&pk).await?;
            let pk_clone = pk.clone();
            actor
                .invoke_async(move |inner| {
                    let fut = inner.persist.store_sign_pair(pk_clone, sk);
                    Ok(ghost_actor::resp(async move {
                        fut.await?;
                        <Result<(), KdError>>::Ok(())
                    }))
                })
                .await?;

            Ok(pk)
        })
    }

    fn sign(
        &self,
        pk: KdHash,
        data: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<Arc<[u8; 64]>, KdError> {
        let actor = self.0.clone();
        ghost_actor::resp(async move {
            let sk = actor
                .invoke_async(move |inner| Ok(inner.persist.get_sign_secret(pk)))
                .await?;
            let mut sig = Buffer::new(64);
            sign::sign_detached(&mut sig, &data, &sk).await?;
            let mut out = [0; 64];
            out.copy_from_slice(&*sig.read_lock());
            Ok(Arc::new(out))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sign_and_verify() -> KdResult<()> {
        let persist = spawn_persist_sqlcipher(KdConfig {
            persist_path: None,
            unlock_passphrase: sodoken::Buffer::new_memlocked(4)?,
            directives: vec![],
        })
        .await?;
        let keystore = spawn_keystore(persist);

        let pk = keystore.generate_sign_agent().await?;
        let data = Buffer::from_ref(b"test");
        let sig = keystore.sign(pk.clone(), data.clone()).await?;
        assert!(pk.verify_signature(data, sig).await);

        Ok(())
    }
}
