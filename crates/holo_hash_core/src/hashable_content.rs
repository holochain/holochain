use crate::HashType;
use holochain_serialized_bytes::prelude::*;

pub trait HashableContent: Sized + Send + Sync
where
    Self: TryInto<SerializedBytes, Error = SerializedBytesError>,
{
    type HashType: HashType;

    fn hash_type(&self) -> Self::HashType;
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
        }
        impl HashableContent for &$n {
            type HashType = holo_hash_core::hash_type::$t;

            fn hash_type(&self) -> Self::HashType {
                use holo_hash_core::PrimitiveHashType;
                holo_hash_core::hash_type::$t::new()
            }
        }
    };
}
