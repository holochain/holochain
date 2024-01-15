//! Definition of the HasHash trait

use crate::hash::HashSerializer;
use crate::HashType;
use crate::HoloHash;

/// Anything which has an owned HoloHashOf.
pub trait HasHash<T: HashType, S: HashSerializer> {
    /// Get the hash by reference
    fn as_hash(&self) -> &HoloHash<T, S>;

    /// Convert to the owned hash
    fn into_hash(self) -> HoloHash<T, S>;
}
