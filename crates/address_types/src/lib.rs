use holochain_serialized_bytes::prelude::*;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Eq, Hash)]
pub struct Address(Vec<u8>);

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
