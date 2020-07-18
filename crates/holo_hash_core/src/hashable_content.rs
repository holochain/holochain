use crate::HashType;
use futures::FutureExt;
use holochain_serialized_bytes::prelude::*;
use must_future::MustBoxFuture;

pub trait HashableContent: Sized + Send + Sync {
    type HashType: HashType;

    /// The HashType which this content will be hashed to
    fn hash_type(&self) -> Self::HashType;

    /// Return a subset of the content as SerializedBytes, which will be used
    /// to compute the hash. This function is only used in the provided
    /// `hashable_bytes` method and should never be called directly.
    fn hashable_content(&self) -> SerializedBytes;

    // /// Provided function to get the actual hash bytes.
    // /// Can be overridden in special cases such as AgentPubKey, which is
    // /// simultaneously content and hash unto itself
    // #[cfg(feature = "string-encoding")]
    // fn hashable_bytes<'a>(&'a self) -> MustBoxFuture<'a, Vec<u8>> {
    //     use crate::encode;
    //     async move {
    //         let bytes: Vec<u8> =
    //             holochain_serialized_bytes::UnsafeBytes::from(&self.hashable_content()).into();
    //         encode::blake2b_256(&bytes)
    //     }
    //     .boxed()
    //     .into()
    // }
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
        // impl HashableContent for &$n {
        //     type HashType = holo_hash_core::hash_type::$t;

        //     fn hash_type(&self) -> Self::HashType {
        //         use holo_hash_core::PrimitiveHashType;
        //         holo_hash_core::hash_type::$t::new()
        //     }

        //     fn hashable_content(self) -> SerializedBytes {
        //         self.try_into()
        //             .expect("Could not serialize HashableContent")
        //     }
        // }
    };
}
