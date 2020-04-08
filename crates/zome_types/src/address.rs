use crate::entry::Entry;
pub use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::SerializedBytes;
use multihash::Blake2b256;

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

impl Address {

    pub fn new(bytes: Vec<u8>) -> Address {
        Address(bytes)
    }

    pub fn generate_from_bytes(bytes: &[u8]) -> Address {
        Address(Blake2b256::digest(bytes).into_bytes())
    }

}

impl Addressable for SerializedBytes {
    fn address(&self) -> Address {
        Address::generate_from_bytes(self.bytes())
    }
}

impl Addressable for Entry {
    fn address(&self) -> Address {
        match &self {
            Entry::AgentId(agent_id) => agent_id.address(),
            _ => Address::generate_from_bytes(
                SerializedBytes::try_from(self)
                    .expect("tried to address an entry that is not serializable")
                    .bytes(),
            ),
        }
    }
}

#[macro_export]
/// implement Addressable for someting that can TryFrom SerializedBytes by dirctly addressing the
/// serialized bytes
macro_rules! serial_address {
    ( $t:ty ) => {
        impl $crate::address::Addressable for $t {
            fn address(&self) -> $crate::address::Address {
                $crate::prelude::SerializedBytes::try_from(self).unwrap().address()
            }
        }
    };
}
