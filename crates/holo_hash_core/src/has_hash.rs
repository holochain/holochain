//! Definition of the HasHash trait

use crate::{HashType, HoloHashImpl};

/// Anything which has an owned HoloHashOf.
pub trait HasHash<T: HashType> {
    /// Get the hash by reference
    // TODO: maybe rename to as_hash
    fn hash(&self) -> &HoloHashImpl<T>;

    /// Convert to the owned hash
    fn into_hash(self) -> HoloHashImpl<T>;
}
