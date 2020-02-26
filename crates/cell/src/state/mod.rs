use sx_state::buffer::KvBuffer;
use sx_types::prelude::Address;

pub struct ChainSequence<'e>(KvBuffer<'e, String, ChainSequenceItem>);

impl<'e> ChainSequence<'e> {
    pub fn chain_head(&self) -> Address {
        unimplemented!()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_address: Address
}
