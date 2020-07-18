use crate::HashType;
use futures::FutureExt;
use holochain_serialized_bytes::prelude::*;
use must_future::MustBoxFuture;

pub trait HashableContent: Sized + Send + Sync {
    type HashType: HashType;

    /// The HashType which this content will be hashed to
    fn hash_type(&self) -> Self::HashType;

    /// Return a subset of the content, either as SerializedBytes "content",
    /// which will be used to compute the hash, or as an already precomputed
    /// hash which will be used directly
    fn hashable_content(&self) -> HashableContentBytes;
}

pub enum HashableContentBytes {
    Content(SerializedBytes),
    Prehashed36(Vec<u8>),
}

#[macro_export]
macro_rules! impl_hashable_content {
    ($n: ident, $t: ident) => {
        impl HashableContent for $n {
            type HashType = holo_hash_core::hash_type::$t;

            fn hash_type(&self) -> Self::HashType {
                use holo_hash_core::PrimitiveHashType;
                holo_hash_core::hash_type::$t::new()
            }

            fn hashable_content(&self) -> $crate::HashableContentBytes {
                $crate::HashableContentBytes::Content(
                    self.try_into()
                        .expect("Could not serialize HashableContent"),
                )
            }
        }
    };
}
