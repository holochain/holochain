use sx_state::{
    buffer::{KvIntBuffer, StoreBuffer},
    db::{DbManager, DbName, CHAIN_SEQUENCE},
    error::{WorkspaceError, WorkspaceResult},
    RkvEnv, Writer, Reader,
};
use sx_types::prelude::Address;

pub struct ChainSequenceBuffer<'e> {
    db: KvIntBuffer<'e, u32, ChainSequenceItem>,
    next_index: u32,
    tx_seq: u32,
}

impl<'e> ChainSequenceBuffer<'e> {
    // TODO: pass store in directly
    pub fn new(reader: &'e Reader<'e>, dbm: &'e DbManager<'e>) -> WorkspaceResult<Self> {
        let db: KvIntBuffer<'e, u32, ChainSequenceItem> =
            KvIntBuffer::new(reader, dbm.get(&*CHAIN_SEQUENCE)?.clone())?;
        let latest = db.iter_reverse()?.next();
        let (next_index, tx_seq) = latest
            .map(|item| (item.index, item.tx_seq))
            .unwrap_or((0, 0));
        Ok(Self {
            db,
            next_index,
            tx_seq,
        })
    }

    pub fn chain_head(&self) -> WorkspaceResult<Option<Address>> {
        Ok(self
            .db
            .iter_reverse()?
            .next()
            .map(|item| item.header_address))
    }

    pub fn add_header(&mut self, header_address: Address) {
        self.db.put(
            self.next_index,
            ChainSequenceItem {
                header_address,
                index: self.next_index,
                tx_seq: self.tx_seq,
                dht_transforms_complete: false,
            },
        );
        self.next_index += 1;
    }
}

impl<'env> StoreBuffer<'env> for ChainSequenceBuffer<'env> {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        self.db.finalize(writer)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_address: Address,
    index: u32, // TODO: this is the key, so once iterators can return keys, we can remove this
    tx_seq: u32,
    dht_transforms_complete: bool,
}
