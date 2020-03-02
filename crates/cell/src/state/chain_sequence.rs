/// The ChainSequence database serves several purposes:
/// - enables fast forward iteration over the entire source chain
/// - knows what the chain head is, by accessing the last item's header address
/// - stores information about which headers were committed in the same transactional bundle
/// - stores info about whether each entry has undergone DHT op generation and publishing
use sx_state::{
    buffer::{KvIntBuffer, StoreBuffer},
    db::{DbManager, DbName, CHAIN_SEQUENCE},
    error::{WorkspaceError, WorkspaceResult},
    Reader, RkvEnv, Writer,
};
use sx_types::prelude::Address;

/// A Value in the ChainSequence database.
#[derive(Clone, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_address: Address,
    index: u32, // TODO: this is the key, so once iterators can return keys, we can remove this
    tx_seq: u32,
    dht_transforms_complete: bool,
}

pub struct ChainSequenceBuffer<'e> {
    db: KvIntBuffer<'e, u32, ChainSequenceItem>,
    next_index: u32,
    tx_seq: u32,
}

impl<'e> ChainSequenceBuffer<'e> {
    pub fn new(reader: &'e Reader<'e>, dbm: &'e DbManager<'e>) -> WorkspaceResult<Self> {
        let db: KvIntBuffer<'e, u32, ChainSequenceItem> =
            KvIntBuffer::new(reader, dbm.get(&*CHAIN_SEQUENCE)?.clone())?;
        let latest = db.iter_raw_reverse()?.next();
        let (next_index, tx_seq) = latest
            .map(|(_, item)| (item.index + 1, item.tx_seq + 1))
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
            .iter_raw_reverse()?
            .next()
            .map(|(_, item)| item.header_address))
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

#[cfg(test)]
pub mod tests {

    use super::{ChainSequenceBuffer, StoreBuffer};
    use sx_state::{
        db::{DbManager, ReadManager, WriteManager},
        env::create_lmdb_env,
        error::WorkspaceResult,
        test_utils::test_env,
    };
    use sx_types::prelude::Address;
    use tempdir::TempDir;

    #[test]
    fn chain_sequence_scratch_awareness() -> WorkspaceResult<()> {
        let arc = test_env();
        let env = arc.read().unwrap();
        let dbm = DbManager::new(&env)?;
        let rm = ReadManager::new(&env);
        let wm = WriteManager::new(&env);
        rm.with_reader(|reader| {
            let mut buf = ChainSequenceBuffer::new(&reader, &dbm)?;
            assert_eq!(buf.chain_head()?, None);
            buf.add_header(Address::from("0"));
            assert_eq!(buf.chain_head()?, Some(Address::from("0")));
            buf.add_header(Address::from("1"));
            assert_eq!(buf.chain_head()?, Some(Address::from("1")));
            buf.add_header(Address::from("2"));
            assert_eq!(buf.chain_head()?, Some(Address::from("2")));
            Ok(())
        })?;

        Ok(())
    }

    #[test]
    fn chain_sequence_functionality() -> WorkspaceResult<()> {
        let arc = test_env();
        let env = arc.read().unwrap();
        let dbm = DbManager::new(&env)?;
        let rm = ReadManager::new(&env);
        let wm = WriteManager::new(&env);
        rm.with_reader(|reader| {
            let mut buf = ChainSequenceBuffer::new(&reader, &dbm)?;
            buf.add_header(Address::from("0"));
            buf.add_header(Address::from("1"));
            assert_eq!(buf.chain_head()?, Some(Address::from("1")));
            buf.add_header(Address::from("2"));
            wm.with_writer(|mut writer| buf.finalize(&mut writer))?;
            Ok(())
        })?;

        rm.with_reader(|reader| {
            let buf = ChainSequenceBuffer::new(&reader, &dbm)?;
            assert_eq!(buf.chain_head()?, Some(Address::from("2")));
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(_, i)| i.index).collect();
            assert_eq!(items, vec![0, 1, 2]);
            Ok(())
        })?;

        rm.with_reader(|reader| {
            let mut buf = ChainSequenceBuffer::new(&reader, &dbm)?;
            buf.add_header(Address::from("3"));
            buf.add_header(Address::from("4"));
            buf.add_header(Address::from("5"));
            wm.with_writer(|mut writer| buf.finalize(&mut writer))?;
            Ok(())
        })?;

        rm.with_reader(|reader| {
            let buf = ChainSequenceBuffer::new(&reader, &dbm)?;
            assert_eq!(buf.chain_head()?, Some(Address::from("5")));
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(_, i)| i.tx_seq).collect();
            assert_eq!(items, vec![0, 0, 0, 1, 1, 1]);
            Ok(())
        })?;

        Ok(())
    }
}
