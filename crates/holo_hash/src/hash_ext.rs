use crate::{
    encode, HashType, HashableContent, HashableContentBytes, HoloHash, PrimitiveHashType,
    HASH_CORE_LEN, HASH_SERIALIZED_LEN,
};

impl<T: HashType> HoloHash<T> {
    /// Hash the given content to produce a HoloHash
    pub fn from_data<'a, C: 'a + HashableContent<HashType = T>>(content: C) -> HoloHash<T> {
        match content.hashable_content() {
            HashableContentBytes::Content(sb) => {
                assert!(sb.bytes().len() <= crate::MAX_HASHABLE_CONTENT_LEN);
                let bytes: Vec<u8> = holochain_serialized_bytes::UnsafeBytes::from(sb).into();
                Self::with_pre_hashed_typed(encode::blake2b_256(&bytes), content.hash_type())
            }
            HashableContentBytes::Prehashed36(bytes) => {
                HoloHash::from_raw_bytes_and_type(bytes, content.hash_type())
            }
        }
    }

    /// Hash a reference to the given content to produce a HoloHash
    pub fn with_data<'a, C: HashableContent<HashType = T>>(content: &'a C) -> HoloHash<T> {
        match content.hashable_content() {
            HashableContentBytes::Content(sb) => {
                assert!(sb.bytes().len() <= crate::MAX_HASHABLE_CONTENT_LEN);
                let bytes: Vec<u8> = holochain_serialized_bytes::UnsafeBytes::from(sb).into();
                Self::with_pre_hashed_typed(encode::blake2b_256(&bytes), content.hash_type())
            }
            HashableContentBytes::Prehashed36(bytes) => {
                HoloHash::from_raw_bytes_and_type(bytes, content.hash_type())
            }
        }
    }

    /// Construct a HoloHash from a prehashed raw 36-byte slice, with given type.
    /// The location bytes will be calculated.
    // TODO: revisit when changing serialization format [ B-02112 ]
    pub fn with_pre_hashed_typed(mut hash: Vec<u8>, hash_type: T) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.

        if hash.len() == HASH_CORE_LEN {
            tracing::warn!("Got core 32 bytes instead of 36, recalcuating loc.");
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
            tracing::warn!("Got core 32 bytes instead of 36, recalcuating loc.");
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
