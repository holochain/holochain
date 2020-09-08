use crate::{
    encode, hash_type, HashType, HashableContent, HashableContentBytes, HoloHash, HoloHashOf,
    HoloHashed, PrimitiveHashType, HASH_CORE_LEN, HASH_SERIALIZED_LEN,
};
use hash_type::{HashTypeAsync, HashTypeSync};

/// The maximum size to hash synchronously. Anything larger than this will
/// take too long to hash within a single tokio context
pub const MAX_HASHABLE_CONTENT_LEN: usize = 16 * 1000 * 1000; // 16 MiB

impl<T: HashType> HoloHash<T> {
    /// Construct a HoloHash from a prehashed raw 36-byte slice, with given type.
    /// The location bytes will be calculated.
    // TODO: revisit when changing serialization format [ B-02112 ]
    pub fn with_pre_hashed_typed(mut hash: Vec<u8>, hash_type: T) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.

        if hash.len() == HASH_CORE_LEN {
            //tracing::warn!("Got core 32 bytes instead of 36, recalcuating loc.");
            hash.append(&mut encode::holo_dht_location_bytes(&hash));
        }

        assert_eq!(
            HASH_SERIALIZED_LEN,
            hash.len(),
            "only 36 byte hashes supported"
        );

        HoloHash::from_raw_bytes_and_type(hash, hash_type)
    }
}

impl<P: PrimitiveHashType> HoloHash<P> {
    /// Construct a HoloHash from a prehashed raw 32-byte slice.
    /// The location bytes will be calculated.
    // TODO: revisit when changing serialization format [ B-02112 ]
    pub fn with_pre_hashed(mut hash: Vec<u8>) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.
        // TODO: DRY, write in terms of with_pre_hashed_typed

        if hash.len() == HASH_CORE_LEN {
            //tracing::warn!("Got core 32 bytes instead of 36, recalcuating loc.");
            hash.append(&mut encode::holo_dht_location_bytes(&hash));
        }

        assert_eq!(
            HASH_SERIALIZED_LEN,
            hash.len(),
            "only 36 byte hashes supported"
        );

        HoloHash::from_raw_bytes(hash)
    }
}

impl<T: HashTypeSync> HoloHash<T> {
    /// Synchronously hash a reference to the given content to produce a HoloHash
    /// If the content is larger than MAX_HASHABLE_CONTENT_LEN, this will **panic**!
    pub fn with_data_sync<C: HashableContent<HashType = T>>(content: &C) -> HoloHash<T> {
        match content.hashable_content() {
            HashableContentBytes::Content(sb) => {
                let bytes: Vec<u8> = holochain_serialized_bytes::UnsafeBytes::from(sb).into();
                if bytes.len() > crate::MAX_HASHABLE_CONTENT_LEN {
                    panic!("Attempted to synchronously hash data larger than the size limit.\nData size: {}\nLimit: {}", bytes.len(), crate::MAX_HASHABLE_CONTENT_LEN);
                }
                Self::with_pre_hashed_typed(encode::blake2b_256(&bytes), content.hash_type())
            }
            HashableContentBytes::Prehashed36(bytes) => {
                HoloHash::from_raw_bytes_and_type(bytes, content.hash_type())
            }
        }
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
}

impl<T: HashTypeAsync> HoloHash<T> {
    /// Asynchronously hash a reference to the given content to produce a HoloHash
    pub async fn with_data<C: HashableContent<HashType = T>>(content: &C) -> HoloHash<T> {
        match content.hashable_content() {
            HashableContentBytes::Content(sb) => {
                let bytes: Vec<u8> = holochain_serialized_bytes::UnsafeBytes::from(sb).into();
                Self::with_pre_hashed_typed(encode::blake2b_256(&bytes), content.hash_type())
            }
            HashableContentBytes::Prehashed36(bytes) => {
                HoloHash::from_raw_bytes_and_type(bytes, content.hash_type())
            }
        }
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
}
