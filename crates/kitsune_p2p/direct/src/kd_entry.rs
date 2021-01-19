//! Kitsune P2p Direct one entry to rule them all

use crate::*;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

fn epoch_ms_to_chrono(epoch_ms: u64) -> DateTime<Utc> {
    let epoch = DateTime::from_utc(NaiveDate::from_ymd(1970, 1, 1).and_hms(0, 0, 0), Utc);
    let duration = chrono::Duration::from_std(std::time::Duration::from_millis(epoch_ms)).unwrap();
    epoch + duration
}

fn chrono_to_epoch_ms(d: DateTime<Utc>) -> u64 {
    let epoch = DateTime::from_utc(NaiveDate::from_ymd(1970, 1, 1).and_hms(0, 0, 0), Utc);
    (d - epoch).to_std().unwrap().as_millis() as u64
}

macro_rules! _repr_enum {
    (#[doc = $ndoc:literal] pub enum $n:ident {
        $(#[doc = $idoc:literal] $i:ident = $l:literal,)*
    }) => {
        #[doc = $ndoc]
        #[repr(u8)]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum $n {$(
            #[doc = $idoc]
            $i = $l,
        )*}

        impl From<u8> for SysType {
            fn from(b: u8) -> Self {
                match b {$(
                    $l => SysType::$i,
                )*
                    _ => panic!("invalid sys_type byte"),
                }
            }
        }

        impl From<SysType> for u8 {
            fn from(s: SysType) -> Self {
                s as u8
            }
        }
    };
}

_repr_enum! {
    /// sys_type enum
    pub enum SysType {
        /// imaginary origin type - no data should actually contain this sys_type
        Origin = 0x00,

        /// hot spot mitigator
        HSM = 0x01,

        /// validation
        Validation = 0x02,

        /// user interface
        UI = 0x03,

        /// authorization
        Auth = 0x10,

        /// app node create
        Create = 0x20,

        /// delete
        Delete = 0x21,
    }
}

pub(crate) struct KdArcSwap<T>(ArcSwapOption<T>)
where
    T: std::fmt::Debug + Clone + PartialEq + Eq;

impl<T> KdArcSwap<T>
where
    T: std::fmt::Debug + Clone + PartialEq + Eq,
{
    pub fn new() -> Self {
        Self(ArcSwapOption::new(None))
    }
}

impl<T> std::fmt::Debug for KdArcSwap<T>
where
    T: std::fmt::Debug + Clone + PartialEq + Eq,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.load())
    }
}

impl<T> std::cmp::PartialEq for KdArcSwap<T>
where
    T: std::fmt::Debug + Clone + PartialEq + Eq,
{
    fn eq(&self, other: &Self) -> bool {
        *self.0.load() == *other.0.load()
    }
}

impl<T> std::cmp::Eq for KdArcSwap<T> where T: std::fmt::Debug + Clone + PartialEq + Eq {}

type KdEntryInner = (
    Box<[u8]>,
    KdArcSwap<serde_json::Value>,
    KdArcSwap<serde_json::Value>,
);

/// Kitsune P2p Direct one entry to rule them all
#[derive(Clone, PartialEq, Eq)]
pub struct KdEntry(Arc<KdEntryInner>);

impl From<Box<[u8]>> for KdEntry {
    fn from(v: Box<[u8]>) -> Self {
        Self(Arc::new((v, KdArcSwap::new(), KdArcSwap::new())))
    }
}

impl std::ops::Deref for KdEntry {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

impl AsRef<[u8]> for KdEntry {
    fn as_ref(&self) -> &[u8] {
        &self.0 .0
    }
}

impl std::borrow::Borrow<[u8]> for KdEntry {
    fn borrow(&self) -> &[u8] {
        &self.0 .0
    }
}

impl std::fmt::Debug for KdEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sig = base64::encode_config(&self.signature()[..], base64::URL_SAFE_NO_PAD);
        f.debug_struct("KdEntry")
            .field("size", &self.size())
            .field("hash", &self.hash())
            .field("signature", &sig)
            .field("sys_type", &self.sys_type())
            .field("create", &self.create())
            .field("expire", &self.expire())
            .field("author", &self.author())
            .field("left_link", &self.left_link())
            .field("right_link", &self.right_link())
            .field("user_type", &self.user_type())
            .field("content", &self.content())
            .finish()
    }
}

const SIZE_START: usize = 0;
const SIZE_LEN: usize = 4;
const HASH_START: usize = 4;
const HASH_LEN: usize = 36;
const SIG_START: usize = 40;
const SIG_LEN: usize = 64;
const SYS_TYPE_START: usize = 104;
const CREATE_START: usize = 105;
const CREATE_LEN: usize = 8;
const EXPIRE_START: usize = 113;
const EXPIRE_LEN: usize = 8;
const AUTHOR_START: usize = 121;
const AUTHOR_LEN: usize = 36;
const LEFT_LINK_START: usize = 157;
const LEFT_LINK_LEN: usize = 36;
const RIGHT_LINK_START: usize = 193;
const RIGHT_LINK_LEN: usize = 36;
const USER_TYPE_START: usize = 229;
const USER_TYPE_LEN: usize = 32;
const CONTENT_START: usize = 261;

macro_rules! _impl_getters {
    ($i:ident) => {
        impl $i {
            /// size/length of underlying raw bytes
            pub fn size(&self) -> u32 {
                self.0 .0.len() as u32
            }

            /// the content portion used for signatures / hashing
            pub fn sig_content(&self) -> &[u8] {
                &self.0 .0[SYS_TYPE_START..]
            }

            /// hash
            pub fn hash(&self) -> KdHash {
                let r = arrayref::array_ref![self.0 .0, HASH_START, HASH_LEN];
                KdHash::from_36_bytes_unchecked(r)
            }

            /// signature bytes
            pub fn signature(&self) -> &[u8; SIG_LEN] {
                arrayref::array_ref![self.0 .0, SIG_START, SIG_LEN]
            }

            /// sys_type
            pub fn sys_type(&self) -> SysType {
                self.0 .0[SYS_TYPE_START].into()
            }

            /// create time in epoch millis
            pub fn create(&self) -> DateTime<Utc> {
                let ms = (&self.0 .0[CREATE_START..CREATE_START + CREATE_LEN])
                    .read_u64::<LittleEndian>()
                    .unwrap();
                epoch_ms_to_chrono(ms)
            }

            /// expire time in epoch millis
            pub fn expire(&self) -> DateTime<Utc> {
                let ms = (&self.0 .0[EXPIRE_START..EXPIRE_START + EXPIRE_LEN])
                    .read_u64::<LittleEndian>()
                    .unwrap();
                epoch_ms_to_chrono(ms)
            }

            /// author
            pub fn author(&self) -> KdHash {
                let r = arrayref::array_ref![self.0 .0, AUTHOR_START, AUTHOR_LEN];
                KdHash::from_36_bytes_unchecked(r)
            }

            /// left_link
            pub fn left_link(&self) -> KdHash {
                let r = arrayref::array_ref![self.0 .0, LEFT_LINK_START, LEFT_LINK_LEN];
                KdHash::from_36_bytes_unchecked(r)
            }

            /// right_link
            pub fn right_link(&self) -> KdHash {
                let r = arrayref::array_ref![self.0 .0, RIGHT_LINK_START, RIGHT_LINK_LEN];
                KdHash::from_36_bytes_unchecked(r)
            }

            /// user_type
            pub fn user_type(&self) -> serde_json::Value {
                if let Some(res) = &*self.0 .1 .0.load() {
                    return (&**res).clone();
                }
                let res: serde_json::Value = (|| {
                    let len = self.0 .0[USER_TYPE_START] as usize;
                    if len > 31 || len == 0 {
                        return serde_json::Value::Null;
                    }
                    let bytes = &self.0 .0[USER_TYPE_START + 1..USER_TYPE_START + 1 + len];
                    match serde_json::from_slice(bytes) {
                        Ok(v) => v,
                        Err(e) => {
                            println!("READ USER TYPE ERROR: {:?}", e);
                            serde_json::Value::Null
                        }
                    }
                })();
                self.0 .1 .0.store(Some(Arc::new(res.clone())));
                res
            }

            /// content
            pub fn content(&self) -> serde_json::Value {
                if let Some(res) = &*self.0 .2 .0.load() {
                    return (&**res).clone();
                }
                let res: serde_json::Value = {
                    let bytes = &self.0 .0[CONTENT_START..];
                    match serde_json::from_slice(bytes) {
                        Ok(v) => v,
                        Err(_) => serde_json::Value::Null,
                    }
                };
                self.0 .2 .0.store(Some(Arc::new(res.clone())));
                res
            }
        }
    };
}

impl KdEntry {
    /// Validated load from raw bytes
    pub async fn from_raw_bytes_validated(b: Box<[u8]>) -> KdResult<Self> {
        let entry = Self(Arc::new((b, KdArcSwap::new(), KdArcSwap::new())));

        // check the size data
        if entry.size() as usize != entry.sig_content().len() + SYS_TYPE_START {
            return Err(format!("invalid size data: {}", entry.size()).into());
        }

        // check the total size is within bounds
        if entry.size() > 1024 * 1024 {
            return Err(format!(
                "entry must fit within 1MiB, content must be != {} bytes",
                1024 * 1024 - CONTENT_START
            )
            .into());
        }

        // check the content/sig hash
        let hash = KdHash::from_data(entry.sig_content()).await?;
        if hash != entry.hash() {
            return Err("Invalid entry hash".into());
        }

        // check the signature
        let data = sodoken::Buffer::from_ref(entry.sig_content());
        if !entry
            .author()
            .verify_signature(data, Arc::new(*entry.signature()))
            .await
        {
            return Err("Invalid signature".into());
        }

        // check expire
        let now = Utc::now();
        if entry.expire() < now {
            return Err(format!("entry expired {}", entry.expire()).into());
        }

        // TODO Validation / size setting, etc
        Ok(entry)
    }

    /// create a new builder for KdEntry instances
    pub fn builder() -> KdEntryBuilder {
        KdEntryBuilder::default()
    }
}

_impl_getters!(KdEntry);

/// Builder for KdEntry struct instances
pub struct KdEntryBuilder(
    (
        Vec<u8>,
        KdArcSwap<serde_json::Value>,
        KdArcSwap<serde_json::Value>,
    ),
);

impl Default for KdEntryBuilder {
    fn default() -> Self {
        Self((vec![0; CONTENT_START], KdArcSwap::new(), KdArcSwap::new()))
    }
}

_impl_getters!(KdEntryBuilder);

impl KdEntryBuilder {
    /// convert this builder into a KdEntry instance
    pub fn build(
        self,
        pub_key: KdHash,
        kd: KitsuneDirect,
    ) -> ghost_actor::GhostFuture<KdEntry, KdError> {
        ghost_actor::resp(async move {
            let mut this = self;

            this = this.set_create(Utc::now());

            this = this.set_author(&pub_key);

            let hash = KdHash::from_data(this.sig_content()).await?;
            let hash = hash.get_raw_bytes();
            let hash = arrayref::array_ref![hash, 3, 36];
            this = this.set_hash(hash);

            let sig = kd
                .sign(
                    pub_key.clone(),
                    sodoken::Buffer::from_ref(this.sig_content()),
                )
                .await?;
            this = this.set_signature(&sig);

            KdEntry::from_raw_bytes_validated(this.0 .0.into_boxed_slice()).await
        })
    }

    /// set the hash data of this instance
    /// PRIVATE since we really only should do this as part of build
    fn set_hash(mut self, hash: &[u8; HASH_LEN]) -> Self {
        self.0 .0[HASH_START..HASH_START + HASH_LEN].copy_from_slice(hash);
        self
    }

    /// set the signature data of this instance
    /// PRIVATE since we really only should do this as part of build
    fn set_signature(mut self, signature: &[u8; SIG_LEN]) -> Self {
        self.0 .0[SIG_START..SIG_START + SIG_LEN].copy_from_slice(signature);
        self
    }

    /// set the sys_type of this instance
    pub fn set_sys_type(mut self, sys_type: SysType) -> Self {
        self.0 .0[SYS_TYPE_START] = sys_type as u8;
        self
    }

    /// set the create data of this instance
    /// PRIVATE since we really only should do this as part of build
    fn set_create(mut self, create: DateTime<Utc>) -> Self {
        let ms = chrono_to_epoch_ms(create);
        (&mut self.0 .0[CREATE_START..CREATE_START + CREATE_LEN])
            .write_u64::<LittleEndian>(ms)
            .unwrap();
        self
    }

    /// set the expire data of this instance
    pub fn set_expire(mut self, expire: DateTime<Utc>) -> Self {
        let ms = chrono_to_epoch_ms(expire);
        (&mut self.0 .0[EXPIRE_START..EXPIRE_START + EXPIRE_LEN])
            .write_u64::<LittleEndian>(ms)
            .unwrap();
        self
    }

    /// set the author data of this instance
    /// PRIVATE since we really only should do this as part of build
    fn set_author(mut self, author: &KdHash) -> Self {
        let author = &author.get_raw_bytes()[3..];
        self.0 .0[AUTHOR_START..AUTHOR_START + AUTHOR_LEN].copy_from_slice(author);
        self
    }

    /// set the left_link data of this instance
    pub fn set_left_link(mut self, left_link: &KdHash) -> Self {
        let left_link = &left_link.get_raw_bytes()[3..];
        self.0 .0[LEFT_LINK_START..LEFT_LINK_START + LEFT_LINK_LEN].copy_from_slice(left_link);
        self
    }

    /// set the right_link data of this instance
    pub fn set_right_link(mut self, right_link: &KdHash) -> Self {
        let right_link = &right_link.get_raw_bytes()[3..];
        self.0 .0[RIGHT_LINK_START..RIGHT_LINK_START + RIGHT_LINK_LEN].copy_from_slice(right_link);
        self
    }

    /// set the user_type data of this instance
    pub fn set_user_type(mut self, user_type: serde_json::Value) -> KdResult<Self> {
        let mut bytes = serde_json::to_vec(&user_type)?;
        if bytes.len() > 31 {
            return Err("user type must fit in 31 bytes".into());
        }
        self.0 .0[USER_TYPE_START] = bytes.len() as u8;
        bytes.resize(USER_TYPE_LEN - 1, 0);
        self.0 .0[USER_TYPE_START + 1..USER_TYPE_START + 1 + USER_TYPE_LEN - 1]
            .copy_from_slice(&bytes);
        self.0 .1 .0.store(Some(Arc::new(user_type)));
        Ok(self)
    }

    /// set the content for this instance
    pub fn set_content(mut self, content: serde_json::Value) -> Self {
        let bytes = serde_json::to_vec(&content).unwrap();
        self.0 .0.truncate(CONTENT_START);
        self.0 .0.extend_from_slice(&bytes);
        let size = self.0 .0.len() as u32;
        (&mut self.0 .0[SIZE_START..SIZE_START + SIZE_LEN])
            .write_u32::<LittleEndian>(size)
            .unwrap();
        self.0 .2 .0.store(Some(Arc::new(content)));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn minimal() -> KdResult<()> {
        let kd = spawn_kitsune_p2p_direct(KdConfig {
            persist_path: None,
            unlock_passphrase: sodoken::Buffer::new_memlocked(4)?,
            directives: vec![],
        })
        .await?;

        let pk = kd.generate_agent().await?;

        let e = KdEntry::builder()
            .set_expire(Utc::now() + chrono::Duration::weeks(2))
            .build(pk, kd)
            .await?;

        println!("{:#?}", e);

        Ok(())
    }

    #[tokio::test]
    async fn content() -> KdResult<()> {
        let kd = spawn_kitsune_p2p_direct(KdConfig {
            persist_path: None,
            unlock_passphrase: sodoken::Buffer::new_memlocked(4)?,
            directives: vec![],
        })
        .await?;

        let pk = kd.generate_agent().await?;

        let other_hash = KdHash::from_bytes(Box::new([0xdb; 32])).await?;

        let e = KdEntry::builder()
            .set_sys_type(SysType::Create)
            .set_expire(Utc::now() + chrono::Duration::weeks(2))
            .set_left_link(&other_hash)
            .set_right_link(&other_hash)
            .set_user_type(serde_json::json!("test-type"))?
            .set_content(serde_json::json!({
                "age": 42,
                "fruit": ["banana", "grape"]
            }))
            .build(pk, kd)
            .await?;

        assert_eq!(serde_json::json!("test-type"), e.user_type());
        assert_eq!(
            serde_json::json!({
                "age": 42,
                "fruit": ["banana", "grape"]
            }),
            e.content()
        );
        assert_eq!(other_hash, e.left_link());
        assert_eq!(other_hash, e.right_link());

        println!("{:#?}", e);

        Ok(())
    }
}
