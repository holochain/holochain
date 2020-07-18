use crate::{HoloHash, HoloHashExt};
use futures::future::FutureExt;
use holo_hash_core::{encode, HashType, HoloHashImpl};
use holochain_serialized_bytes::prelude::*;
use must_future::MustBoxFuture;

pub trait HashableContent: Sized + Send + Sync {
    type HashType: HashType;

    fn hash_type(&self) -> Self::HashType;
    fn hashable_content(&self) -> SerializedBytes;
    fn compute_hash<'a>(&'a self) -> MustBoxFuture<'a, HoloHashImpl<Self::HashType>> {
        async move {
            let sb = self.hashable_content();
            let bytes: Vec<u8> = holochain_serialized_bytes::UnsafeBytes::from(sb).into();
            let hash = HoloHashExt::<Self>::with_pre_hashed_typed(
                encode::blake2b_256(&bytes),
                self.hash_type(),
            );
            hash
        }
        .boxed()
        .into()
    }
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

            fn hashable_content(&self) -> SerializedBytes {
                self.try_into()
                    .expect("Could not serialize HashableContent")
            }
        }
        impl HashableContent for &$n {
            type HashType = holo_hash_core::hash_type::$t;

            fn hash_type(&self) -> Self::HashType {
                use holo_hash_core::PrimitiveHashType;
                holo_hash_core::hash_type::$t::new()
            }
            fn hashable_content(&self) -> SerializedBytes {
                self.try_into()
                    .expect("Could not serialize HashableContent")
            }
        }
    };
}
