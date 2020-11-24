//! Definition of the HasHash trait

use crate::HashType;
use crate::HoloHash;

/// Anything which has an owned HoloHashOf.
pub trait HasHash<T: HashType> {
    /// Get the hash by reference
    fn as_hash(&self) -> &HoloHash<T>;

    /// Convert to the owned hash
    fn into_hash(self) -> HoloHash<T>;
}
