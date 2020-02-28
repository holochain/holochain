use sx_state::{
    buffer::KvIntBuffer,
    db::DbName,
    error::{WorkspaceError, WorkspaceResult},
};
use sx_types::prelude::Address;

pub struct ChainSequenceDb<'e>(KvIntBuffer<'e, u32, ChainSequenceItem>);

impl<'e> ChainSequenceDb<'e> {
    pub fn chain_head(&self) -> WorkspaceResult<Address> {
        match self.0.iter_reverse()?.next() {
            Some(item) => Ok(item.header_address),
            None => Err(WorkspaceError::EmptyStore(DbName::ChainSequence)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_address: Address,
    tx_seq: u32,
    dht_transforms_complete: bool,
}
