use crate::{encode, HashType, HashableContent, HashableContentBytes, HoloHash, PrimitiveHashType};
use futures::FutureExt;
use must_future::MustBoxFuture;

impl<T: HashType> HoloHash<T> {
    /// Hash the given content to produce a HoloHash
    pub fn with_data<'a, C: HashableContent<HashType = T>>(
        content: &'a C,
    ) -> MustBoxFuture<'a, HoloHash<T>> {
        async move {
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
        .boxed()
        .into()
    }

    /// Construct a HoloHash from a prehashed raw 32-byte slice, with given type.
    /// The location bytes will be calculated.
    pub fn with_pre_hashed_typed(mut hash: Vec<u8>, hash_type: T) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.
        assert_eq!(32, hash.len(), "only 32 byte hashes supported");

        hash.append(&mut encode::holo_dht_location_bytes(&hash));
        HoloHash::from_raw_bytes_and_type(hash, hash_type)
    }
}

impl<P: PrimitiveHashType> HoloHash<P> {
    /// Construct a HoloHash from a prehashed raw 32-byte slice.
    /// The location bytes will be calculated.
    pub fn with_pre_hashed(mut hash: Vec<u8>) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.
        // TODO: DRY, write in terms of with_pre_hashed_typed
        assert_eq!(32, hash.len(), "only 32 byte hashes supported");

        hash.append(&mut encode::holo_dht_location_bytes(&hash));
        HoloHash::from_raw_bytes(hash)
    }
}
