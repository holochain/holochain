//! Definition of the HasHash trait

use crate::HashType;
use crate::HoloHash;

/// Anything which has an owned HoloHashOf.
pub trait HasHash {
    /// The type of the hash which is had.
    type HashType: HashType;

    /// Get the hash by reference
    fn as_hash(&self) -> &HoloHash<Self::HashType>;

    /// Convert to the owned hash
    fn into_hash(self) -> HoloHash<Self::HashType>;
}
