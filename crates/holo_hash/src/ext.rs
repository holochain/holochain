use crate::HoloHash;
use futures::FutureExt;
use holo_hash_core::{
    encode, HashableContent, HashableContentBytes, HoloHashImpl, PrimitiveHashType,
};
use holochain_serialized_bytes::prelude::*;
use must_future::MustBoxFuture;

pub trait HoloHashExt<C: HashableContent> // for<'a> &'a C: HashableContent,
{
    fn with_content<'a>(content: &'a C) -> MustBoxFuture<'a, HoloHash<C>>;

    // TODO: deprecate
    // #[deprecated = "alias for with_content"]
    fn with_data<'a>(content: &'a C) -> MustBoxFuture<'a, HoloHash<C>>;

    fn with_pre_hashed_typed(hash: Vec<u8>, hash_type: C::HashType) -> Self;
}

impl<C: HashableContent> HoloHashExt<C> for HoloHash<C>
where
// for<'a> &'a C: HashableContent,
{
    fn with_content<'a>(content: &'a C) -> MustBoxFuture<'a, HoloHash<C>> {
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
                    HoloHashImpl::from_raw_bytes_and_type(bytes, content.hash_type())
                }
            }
        }
        .boxed()
        .into()
    }

    fn with_data<'a>(content: &'a C) -> MustBoxFuture<'a, HoloHash<C>> {
        Self::with_content(content)
    }

    fn with_pre_hashed_typed(mut hash: Vec<u8>, hash_type: C::HashType) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.
        assert_eq!(32, hash.len(), "only 32 byte hashes supported");

        hash.append(&mut encode::holo_dht_location_bytes(&hash));
        HoloHashImpl::from_raw_bytes_and_type(hash, hash_type)
    }
}

pub trait HoloHashPrimitiveExt<P: PrimitiveHashType> {
    fn with_pre_hashed(hash: Vec<u8>) -> Self;
}

impl<P: PrimitiveHashType> HoloHashPrimitiveExt<P> for HoloHashImpl<P> {
    fn with_pre_hashed(mut hash: Vec<u8>) -> Self {
        // Assert the data size is relatively small so we are
        // comfortable executing this synchronously / blocking
        // tokio thread.
        // TODO: DRY, write in terms of with_pre_hashed_typed
        assert_eq!(32, hash.len(), "only 32 byte hashes supported");

        hash.append(&mut encode::holo_dht_location_bytes(&hash));
        HoloHashImpl::from_raw_bytes(hash)
    }
}
