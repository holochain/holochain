use crate::HoloHashOf;
use futures::FutureExt;
use holo_hash::{encode, HashableContent, HashableContentBytes, HoloHash, PrimitiveHashType};
use must_future::MustBoxFuture;

/// Extension trait for HoloHash, which allows instantiation with
/// HashableContent rather than raw bytes
pub trait HoloHashExt<C: HashableContent> {
    /// Hash the given content to produce a HoloHash
    fn with_data<'a>(content: &'a C) -> MustBoxFuture<'a, HoloHashOf<C>>;

    /// Construct a HoloHash from a prehashed raw 32-byte slice, with given type.
    /// The location bytes will be calculated.
    fn with_pre_hashed_typed(hash: Vec<u8>, hash_type: C::HashType) -> Self;
}

impl<C: HashableContent> HoloHashOf<C> {
    pub fn with_data<'a>(content: &'a C) -> MustBoxFuture<'a, HoloHashOf<C>> {
        async move {
            match content.hashable_content() {
                HashableContentBytes::Content(sb) => {
                    let bytes: Vec<u8> = holochain_serialized_bytes::UnsafeBytes::from(sb).into();
                    HoloHashExt::<C>::with_pre_hashed_typed(
                        encode::blake2b_256(&bytes),
                        content.hash_type(),
                    )
                }
                HashableContentBytes::Prehashed36(bytes) => {
                    HoloHash::from_raw_bytes_and_type(bytes, content.hash_type())
                }
            }
        }
        .boxed()
        .into()
    }

    pub fn with_pre_hashed_typed(mut hash: Vec<u8>, hash_type: C::HashType) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.
        assert_eq!(32, hash.len(), "only 32 byte hashes supported");

        hash.append(&mut encode::holo_dht_location_bytes(&hash));
        HoloHash::from_raw_bytes_and_type(hash, hash_type)
    }
}

/// Allows the HashType to be inferred when constructing a hash from raw bytes,
/// if the HashType is primitive
pub trait HoloHashPrimitiveExt<P: PrimitiveHashType> {
    /// Construct a HoloHash from a prehashed raw 32-byte slice.
    /// The location bytes will be calculated.
    fn with_pre_hashed(hash: Vec<u8>) -> Self;
}

impl<P: PrimitiveHashType> HoloHash<P> {
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
