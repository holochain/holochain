//! Defines the prefixes for the various HashTypes, as well as the traits
//! which unify them

mod composite;
mod primitive;
pub use composite::*;
pub use primitive::*;

use crate::error::HoloHashResult;

/// Every HoloHash is generic over HashType.
/// Additionally, every HashableContent has an associated HashType.
/// The HashType is the glue that binds together HashableContent with its hash.
pub trait HashType:
    Copy + Clone + std::fmt::Debug + Clone + std::hash::Hash + PartialEq + Eq + PartialOrd + Ord
{
    /// Get the 3-byte prefix for the underlying primitive hash type
    fn get_prefix(self) -> &'static [u8];

    /// Given a 3-byte prefix, return the corresponding HashType, or error if mismatched.
    /// Trivial for PrimitiveHashType, but useful for composite types
    fn try_from_prefix(prefix: &[u8]) -> HoloHashResult<Self>;

    /// Get a Display-worthy name for this hash type
    fn hash_name(self) -> &'static str;
}

/// HashTypes whose content are hashable synchronously, i.e. the content is guaranteed to be small
pub trait HashTypeSync: HashType {}
/// HashTypes whose content are only hashable asynchronously, i.e. the content is unbounded in size
pub trait HashTypeAsync: HashType {}
