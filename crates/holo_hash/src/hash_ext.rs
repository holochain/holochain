use crate::assert_length;
use crate::encode;
use crate::hash_type;
use crate::HashType;
use crate::HashableContent;
use crate::HashableContentBytes;
use crate::HoloHash;
use crate::HoloHashOf;
use crate::HoloHashed;
use crate::HOLO_HASH_CORE_LEN;
use futures::FutureExt;
use hash_type::HashTypeAsync;
use hash_type::HashTypeSync;
use must_future::MustBoxFuture;

/// The maximum size to hash synchronously. Anything larger than this will
/// take too long to hash within a single tokio context
pub const MAX_HASHABLE_CONTENT_LEN: usize = 16 * 1000 * 1000; // 16 MiB

impl<T: HashTypeSync> HoloHash<T> {
    /// Synchronously hash a reference to the given content to produce a HoloHash
    /// If the content is larger than MAX_HASHABLE_CONTENT_LEN, this will **panic**!
    pub fn with_data_sync<C: HashableContent<HashType = T>>(content: &C) -> HoloHash<T> {
        hash_from_content(content)
    }
}

impl<T, C> HoloHashed<C>
where
    T: HashTypeSync,
    C: HashableContent<HashType = T>,
{
    /// Compute the hash of this content and store it alongside
    pub fn from_content_sync(content: C) -> Self {
        let hash: HoloHashOf<C> = HoloHash::<T>::with_data_sync(&content);
        Self { content, hash }
    }

    /// Verify that the cached hash matches the content.
    /// Important to run this after e.g. deserialization.
    pub fn verify_hash_sync(&self) -> Result<(), HoloHash<T>> {
        let hash = HoloHash::<T>::with_data_sync(&self.content);
        if self.hash == hash {
            Ok(())
        } else {
            Err(hash)
        }
    }
}

impl<T: HashTypeAsync> HoloHash<T> {
    /// Asynchronously hash a reference to the given content to produce a HoloHash
    // TODO: this needs to be pushed onto a background thread if the content is large
    pub async fn with_data<C: HashableContent<HashType = T>>(content: &C) -> HoloHash<T> {
        hash_from_content(content)
    }
}

impl<T, C> HoloHashed<C>
where
    T: HashTypeAsync,
    C: HashableContent<HashType = T>,
{
    /// Compute the hash of this content and store it alongside
    pub async fn from_content(content: C) -> Self {
        let hash: HoloHashOf<C> = HoloHash::<T>::with_data(&content).await;
        Self { content, hash }
    }

    /// Verify that the cached hash matches the content.
    /// Important to run this after e.g. deserialization.
    pub async fn verify_hash(&self) -> Result<(), HoloHash<T>> {
        let hash = HoloHash::<T>::with_data(&self.content).await;
        if self.hash == hash {
            Ok(())
        } else {
            Err(hash)
        }
    }
}

impl<T, C> HoloHashed<C>
where
    T: HashTypeAsync,
    C: HashableContent<HashType = T>,
{
}

fn hash_from_content<T: HashType, C: HashableContent<HashType = T>>(content: &C) -> HoloHash<T> {
    match content.hashable_content() {
        HashableContentBytes::Content(sb) => {
            let bytes: Vec<u8> = holochain_serialized_bytes::UnsafeBytes::from(sb).into();
            let hash = encode::blake2b_256(&bytes);
            assert_length!(HOLO_HASH_CORE_LEN, &hash);
            HoloHash::<T>::from_raw_32_and_type(hash, content.hash_type())
        }
        HashableContentBytes::Prehashed39(bytes) => HoloHash::from_raw_39_panicky(bytes),
    }
}

/// Adds convenience methods for constructing HoloHash and HoloHashed
/// from some HashableContent
pub trait HashableContentExtSync<T>: HashableContent
where
    T: HashTypeSync,
{
    /// Construct a HoloHash from a reference
    fn to_hash(&self) -> HoloHash<T>;
    /// Move into a HoloHashed
    fn into_hashed(self) -> HoloHashed<Self>;
}

/// Adds convenience methods for constructing HoloHash and HoloHashed
/// from some HashableContent
pub trait HashableContentExtAsync<'a, T>: HashableContent
where
    T: HashTypeAsync,
{
    /// Construct a HoloHash from a reference
    fn to_hash(&self) -> MustBoxFuture<HoloHash<T>>;
    /// Move into a HoloHashed
    fn into_hashed(self) -> MustBoxFuture<'a, HoloHashed<Self>>;
}

impl<T, C> HashableContentExtSync<T> for C
where
    T: HashTypeSync,
    C: HashableContent<HashType = T>,
{
    fn to_hash(&self) -> HoloHash<T> {
        HoloHash::with_data_sync(self)
    }

    fn into_hashed(self) -> HoloHashed<Self> {
        HoloHashed::from_content_sync(self)
    }
}

impl<'a, T, C> HashableContentExtAsync<'a, T> for C
where
    T: HashTypeAsync,
    C: 'a + HashableContent<HashType = T> + Send + Sync,
{
    fn to_hash(&self) -> MustBoxFuture<HoloHash<T>> {
        async move { HoloHash::with_data(self).await }
            .boxed()
            .into()
    }

    fn into_hashed(self) -> MustBoxFuture<'a, HoloHashed<Self>> {
        async move { HoloHashed::from_content(self).await }
            .boxed()
            .into()
    }
}
