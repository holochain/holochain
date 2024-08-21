use hdi::prelude::*;


pub type KeyBytes = [u8; 32];


/// A deterministic entry that contains only the cord 32 bytes of a key
///
/// The core 32 bytes of a registered key is the `AgentPubKey` stripped of the 3 byte multihash
/// prefix and 4 byte DHT location suffix.  The `EntryHash` can be derived so that the status of a
/// key can be looked up in a single `get_details` call.
#[hdk_entry_helper]
#[derive(Clone,PartialEq)]
pub struct KeyAnchor { pub bytes: KeyBytes, }

impl KeyAnchor {
    pub fn new(bytes: KeyBytes) -> Self {
        KeyAnchor {
            bytes,
        }
    }
}


impl TryFrom<AgentPubKey> for KeyAnchor {
    type Error = WasmError;

    fn try_from(input: AgentPubKey) -> Result<Self, Self::Error> {
        Ok(
            Self {
                bytes: input.get_raw_32().try_into()
                    .map_err( |e| wasm_error!(WasmErrorInner::Guest(format!(
                        "Failed AgentPubKey to [u8;32] conversion: {:?}", e
                    ))) )?,
            }
        )
    }
}

impl TryFrom<&AgentPubKey> for KeyAnchor {
    type Error = WasmError;

    fn try_from(input: &AgentPubKey) -> Result<Self, Self::Error> {
        input.to_owned().try_into()
    }
}
