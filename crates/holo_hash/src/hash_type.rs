//! Defines the prefixes for the various HashTypes, as well as the traits
//! which unify them

mod composite;
mod primitive;
pub use composite::*;
pub use primitive::*;

/// Every HoloHash is generic over HashType.
/// Additionally, every HashableContent has an associated HashType.
/// The HashType is the glue that binds together HashableContent with its hash.
pub trait HashType:
    Copy
    + Clone
    + std::fmt::Debug
    + Clone
    + std::hash::Hash
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + serde::de::DeserializeOwned
    + serde::Serialize
    // FIXME: REMOVE!!! This is a hack to get composite keys working with LMDB before [ B-02112 ] is done
    + Default
{
    /// Get the 3 byte prefix for the underlying primitive hash type
    fn get_prefix(self) -> &'static [u8];

    /// Get a Display-worthy name for this hash type
    fn hash_name(self) -> &'static str;
}
