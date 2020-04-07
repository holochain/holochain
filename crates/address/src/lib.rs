pub mod prelude;

use multihash::Blake2b256;

pub fn generate_from_bytes(bytes: &[u8]) -> Address {
    Address(Blake2b256::digest(bytes).into_bytes())
}

impl Addressable for SerializedBytes {
    fn address(&self) -> Address {
        generate_from_bytes(self.bytes())
    }
}

impl Addressable for Entry {
    fn address(&self) -> Address {
        match &self {
            Entry::AgentId(agent_id) => agent_id.address(),
            _ => Address::encode_from_bytes(
                SerializedBytes::try_from(self)
                    .expect("tried to address an entry that is not serializable")
                    .bytes(),
                crate::sx_zome_types::hash::DEFAULT_HASH,
            ),
        }
    }
}

serial_address!(Dna);

#[macro_export]
/// implement Addressable for someting that can TryFrom SerializedBytes by dirctly addressing the
/// serialized bytes
macro_rules! serial_address {
    ( $t:ty ) => {
        impl $crate::persistence::cas::content::Addressable for $t {
            fn address(&self) -> $crate::persistence::cas::content::Address {
                $crate::prelude::SerializedBytes::try_from(self).unwrap().address()
            }
        }
    };
}
