//! Implements a definition of what AddressableContent is by defining Content,
//! defining Address, and defining the relationship between them. AddressableContent is a trait,
//! meaning that it can be implemented for other structs.
//! A test suite for AddressableContent is also implemented here.

use crate::persistence::hash::HashString;
use holochain_serialized_bytes::prelude::*;

use multihash::Hash;

/// an Address for some Content
/// ideally would be the Content but pragmatically must be Address
/// consider what would happen if we had multi GB addresses...
pub type Address = HashString;

/// can be stored as serialized content
/// the content is the address, there is no "location" like a file system or URL
/// @see https://en.wikipedia.org/wiki/Content-addressable_storage
pub trait Addressable {
    /// the Address the Content would be available at once stored in a ContentAddressableStorage
    /// default implementation is provided as hashing Content with sha256
    /// the default implementation should cover most use-cases
    /// it is critical that there are no hash collisions across all stored AddressableContent
    /// it is recommended to implement an "address space" prefix for address algorithms that don't
    /// offer strong cryptographic guarantees like sha et. al.
    fn address(&self) -> Address;
}

impl Addressable for SerializedBytes {
    fn address(&self) -> Address {
        Address::encode_from_bytes(self.bytes(), Hash::SHA2256)
    }
}

#[macro_export]
/// implement Addressable for someting that can TryFrom SerializedBytes
macro_rules! addressable_serializable {
    ( $t:ty ) => {
        impl $crate::persistence::cas::content::Addressable for $t {
            fn address(&self) -> $crate::persistence::cas::content::Address {
                let serialized_bytes = $crate::prelude::SerializedBytes::try_from(self).unwrap();
                $crate::persistence::cas::content::Address::encode_from_bytes(
                    serialized_bytes.bytes(),
                    $crate::persistence::hash::DEFAULT_HASH,
                )
            }
        }
    };
}
