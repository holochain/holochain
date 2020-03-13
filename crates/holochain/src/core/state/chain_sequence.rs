/// The BufferedStore for the Chain Sequence database
/// This database serves several purposes:
/// - enables fast forward iteration over the entire source chain
/// - knows what the chain head is, by accessing the last item's header address
/// - stores information about which headers were committed in the same transactional bundle
/// - stores info about whether each entry has undergone DHT op generation and publishing
///
/// When committing the ChainSequence db, a special step is taken to ensure source chain consistency.
/// If the chain head has moved since the db was created, committing the transaction fails with a special error type.
use crate::core::state::source_chain::{SourceChainError, SourceChainResult};
use serde::{Deserialize, Serialize};
use sx_state::{
    buffer::{BufferedStore, IntKvBuf},
    db::{DbManager, CHAIN_SEQUENCE},
    error::DatabaseResult,
    prelude::{Readable, Writer},
};
use sx_types::prelude::Address;
use tracing::*;

/// A Value in the ChainSequence database.
#[derive(Clone, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_address: Address,
    tx_seq: u32,
    dht_transforms_complete: bool,
}

type Store<'e, R> = IntKvBuf<'e, u32, ChainSequenceItem, R>;

pub struct ChainSequenceBuf<'e, R: Readable> {
    db: Store<'e, R>,
    next_index: u32,
    tx_seq: u32,
    current_head: Option<Address>,
    persisted_head: Option<Address>,
}

impl<'e, R: Readable> ChainSequenceBuf<'e, R> {
    pub fn new(reader: &'e R, dbs: &'e DbManager) -> DatabaseResult<Self> {
        let db: Store<'e, R> = IntKvBuf::new(reader, *dbs.get(&*CHAIN_SEQUENCE)?)?;
        Self::from_db(db)
    }

    pub fn with_reader<RR: Readable>(
        &self,
        reader: &'e RR,
    ) -> DatabaseResult<ChainSequenceBuf<'e, RR>> {
        Self::from_db(self.db.with_reader(reader))
    }

    fn from_db<RR: Readable>(db: Store<'e, RR>) -> DatabaseResult<ChainSequenceBuf<'e, RR>> {
        let latest = db.iter_raw_reverse()?.next();
        let (next_index, tx_seq, current_head) = latest
            .map(|(key, item)| (key + 1, item.tx_seq + 1, Some(item.header_address)))
            .unwrap_or((0, 0, None));
        let persisted_head = current_head.clone();

        Ok(ChainSequenceBuf {
            db,
            next_index,
            tx_seq,
            current_head,
            persisted_head,
        })
    }

    pub fn chain_head(&self) -> Option<&Address> {
        self.current_head.as_ref()
    }

    #[instrument(skip(self))]
    pub fn add_header(&mut self, header_address: Address) {
        self.db.put(
            self.next_index,
            ChainSequenceItem {
                header_address: header_address.clone(),
                tx_seq: self.tx_seq,
                dht_transforms_complete: false,
            },
        );
        trace!(self.next_index);
        self.next_index += 1;
        self.current_head = Some(header_address);
    }
}

impl<'env, R: Readable> BufferedStore<'env> for ChainSequenceBuf<'env, R> {
    type Error = SourceChainError;

    /// Commit to the source chain, performing an as-at check and returning a
    /// SourceChainError::HeadMoved error if the as-at check fails
    fn flush_to_txn(self, writer: &'env mut Writer) -> SourceChainResult<()> {
        let fresh = self.with_reader(writer)?;
        let (old, new) = (self.persisted_head, fresh.persisted_head);
        if old != new {
            Err(SourceChainError::HeadMoved(old, new))
        } else {
            Ok(self.db.flush_to_txn(writer)?)
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::{BufferedStore, ChainSequenceBuf, SourceChainError};
    use crate::core::state::source_chain::SourceChainResult;
    use sx_state::{
        env::{ReadManager, WriteManager},
        error::DatabaseResult,
        test_utils::test_env,
    };
    use sx_types::{observability, prelude::Address};

    #[tokio::test]
    async fn chain_sequence_scratch_awareness() -> DatabaseResult<()> {
        observability::test_run().ok();
        let arc = test_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        env.with_reader(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(buf.chain_head(), None);
            buf.add_header(Address::from("0"));
            assert_eq!(buf.chain_head(), Some(&Address::from("0")));
            buf.add_header(Address::from("1"));
            assert_eq!(buf.chain_head(), Some(&Address::from("1")));
            buf.add_header(Address::from("2"));
            assert_eq!(buf.chain_head(), Some(&Address::from("2")));
            Ok(())
        })
    }

    #[tokio::test]
    async fn chain_sequence_functionality() -> SourceChainResult<()> {
        let arc = test_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        env.with_reader::<SourceChainError, _, _>(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.add_header(Address::from("0"));
            buf.add_header(Address::from("1"));
            assert_eq!(buf.chain_head(), Some(&Address::from("1")));
            buf.add_header(Address::from("2"));
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(buf.chain_head(), Some(&Address::from("2")));
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(key, _)| key).collect();
            assert_eq!(items, vec![0, 1, 2]);
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.add_header(Address::from("3"));
            buf.add_header(Address::from("4"));
            buf.add_header(Address::from("5"));
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(buf.chain_head(), Some(&Address::from("5")));
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(_, i)| i.tx_seq).collect();
            assert_eq!(items, vec![0, 0, 0, 1, 1, 1]);
            Ok(())
        })?;

        Ok(())
    }

    #[tokio::test]
    async fn chain_sequence_head_moved() -> anyhow::Result<()> {
        let arc1 = test_env();
        let arc2 = arc1.clone();
        let (tx1, rx1) = tokio::sync::oneshot::channel();
        let (tx2, rx2) = tokio::sync::oneshot::channel();

        let task1 = tokio::spawn(async move {
            let env = arc1.guard().await;
            let dbs = arc1.clone().dbs().await?;
            let reader = env.reader()?;
            let mut buf = { ChainSequenceBuf::new(&reader, &dbs)? };
            buf.add_header(Address::from("0"));
            buf.add_header(Address::from("1"));
            buf.add_header(Address::from("2"));

            // let the other task run and make a commit to the chain head,
            // which will cause this one to error out when it re-enters and tries to commit
            tx1.send(()).unwrap();
            rx2.await.unwrap();

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        });

        let task2 = tokio::spawn(async move {
            rx1.await.unwrap();
            let env = arc2.guard().await;
            let dbs = arc2.clone().dbs().await?;
            let reader = env.reader()?;
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.add_header(Address::from("3"));
            buf.add_header(Address::from("4"));
            buf.add_header(Address::from("5"));

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            tx2.send(()).unwrap();
            Result::<_, SourceChainError>::Ok(())
        });

        let (result1, result2) = tokio::join!(task1, task2);

        assert_eq!(
            result1.unwrap(),
            Err(SourceChainError::HeadMoved(None, Some(Address::from("5"))))
        );
        assert!(result2.unwrap().is_ok());

        Ok(())
    }
}
