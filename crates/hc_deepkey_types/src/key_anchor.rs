use hdi::prelude::*;

pub type KeyBytes = [u8; 32];

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct KeyAnchor {
    pub bytes: KeyBytes,
}

impl KeyAnchor {
    pub fn new(bytes: KeyBytes) -> Self {
        KeyAnchor { bytes }
    }
}

impl TryFrom<AgentPubKey> for KeyAnchor {
    type Error = WasmError;

    fn try_from(input: AgentPubKey) -> Result<Self, Self::Error> {
        Ok(Self {
            bytes: input.get_raw_32().try_into().map_err(|e| {
                wasm_error!(WasmErrorInner::Guest(format!(
                    "Failed AgentPubKey to [u8;32] conversion: {:?}",
                    e
                )))
            })?,
        })
    }
}

impl TryFrom<&AgentPubKey> for KeyAnchor {
    type Error = WasmError;

    fn try_from(input: &AgentPubKey) -> Result<Self, Self::Error> {
        input.to_owned().try_into()
    }
}
