use crate::HashType;
use holochain_serialized_bytes::prelude::*;

/// Any implementor of HashableContent may be used in a HoloHashed to pair
/// data with its HoloHash representation. It also has an associated HashType.
pub trait HashableContent: Sized {
    /// The HashType which this content will be hashed to
    type HashType: HashType;

    /// The HashType which this content will be hashed to
    fn hash_type(&self) -> Self::HashType;

    /// Return a subset of the content, either as SerializedBytes "content",
    /// which will be used to compute the hash, or as an already precomputed
    /// hash which will be used directly
    fn hashable_content(&self) -> HashableContentBytes;
}

/// HashableContent can be expressed as "content", or "prehashed", which affects
/// how a HoloHashed type will be constructed from it.
pub enum HashableContentBytes {
    /// Denotes that the hash should be computed for the given data
    Content(SerializedBytes),
    /// Denotes that the given bytes already constitute a valid HoloHash
    Prehashed39(Vec<u8>),
}

/// A default HashableContent implementation, suitable for content which
/// is already TryInto<SerializedBytes>, and uses a PrimitiveHashType
#[macro_export]
macro_rules! impl_hashable_content {
    ($n: ident, $t: ident) => {
        impl HashableContent for $n {
            type HashType = holo_hash::hash_type::$t;

            fn hash_type(&self) -> Self::HashType {
                use holo_hash::PrimitiveHashType;
                holo_hash::hash_type::$t::new()
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
