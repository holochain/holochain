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
use holo_hash::HeaderHash;
use serde::{Deserialize, Serialize};
use sx_state::{
    buffer::{BufferedStore, IntKvBuf},
    db::{DbManager, CHAIN_SEQUENCE},
    error::DatabaseResult,
    prelude::{Readable, Writer},
};
use tracing::*;

/// A Value in the ChainSequence database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_hash: HeaderHash,
    tx_seq: u32,
    dht_transforms_complete: bool,
}

type Store<'e, R> = IntKvBuf<'e, u32, ChainSequenceItem, R>;

/// A BufferedStore for interacting with the ChainSequence database
pub struct ChainSequenceBuf<'e, R: Readable> {
    db: Store<'e, R>,
    next_index: u32,
    tx_seq: u32,
    current_head: Option<HeaderHash>,
    persisted_head: Option<HeaderHash>,
}

impl<'e, R: Readable> ChainSequenceBuf<'e, R> {
    /// Create a new instance from a read-only transaction and a database reference
    pub fn new(reader: &'e R, dbs: &'e DbManager) -> DatabaseResult<Self> {
        let db: Store<'e, R> = IntKvBuf::new(reader, *dbs.get(&*CHAIN_SEQUENCE)?)?;
        Self::from_db(db)
    }

    /// Create a new instance from a new read-only transaction, using the same database
    /// as an existing instance. Useful for getting a fresh read-only snapshot of a database.
    pub fn with_reader<RR: Readable>(
        &self,
        reader: &'e RR,
    ) -> DatabaseResult<ChainSequenceBuf<'e, RR>> {
        Self::from_db(self.db.with_reader(reader))
    }

    fn from_db<RR: Readable>(db: Store<'e, RR>) -> DatabaseResult<ChainSequenceBuf<'e, RR>> {
        let latest = db.iter_raw_reverse()?.next();
        debug!("{:?}", latest);
        let (next_index, tx_seq, current_head) = latest
            .map(|(key, item)| (key + 1, item.tx_seq + 1, Some(item.header_hash)))
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

    /// Get the chain head, AKA top chain header. None if the chain is empty.
    pub fn chain_head(&self) -> Option<&HeaderHash> {
        self.current_head.as_ref()
    }

    /// Add a header to the chain, setting all other values automatically.
    /// This is intentionally the only way to modify this database.
    #[instrument(skip(self))]
    pub fn put_header(&mut self, header_hash: HeaderHash) {
        self.db.put(
            self.next_index,
            ChainSequenceItem {
                header_hash: header_hash.clone(),
                tx_seq: self.tx_seq,
                dht_transforms_complete: false,
            },
        );
        trace!(self.next_index);
        self.next_index += 1;
        self.current_head = Some(header_hash);
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
    use holo_hash::holo_hash_core::HeaderHash;
    use sx_state::{
        env::{ReadManager, WriteManager},
        error::DatabaseResult,
        test_utils::test_cell_env,
    };
    use sx_types::observability;

    #[tokio::test]
    async fn chain_sequence_scratch_awareness() -> DatabaseResult<()> {
        observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        env.with_reader(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(buf.chain_head(), None);
            buf.put_header(HeaderHash::new(vec![0]).into());
            assert_eq!(buf.chain_head(), Some(&HeaderHash::new(vec![0]).into()));
            buf.put_header(HeaderHash::new(vec![1]).into());
            assert_eq!(buf.chain_head(), Some(&HeaderHash::new(vec![1]).into()));
            buf.put_header(HeaderHash::new(vec![2]).into());
            assert_eq!(buf.chain_head(), Some(&HeaderHash::new(vec![2]).into()));
            Ok(())
        })
    }

    #[tokio::test]
    async fn chain_sequence_functionality() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.put_header(HeaderHash::new(vec![0]).into());
            buf.put_header(HeaderHash::new(vec![1]).into());
            assert_eq!(buf.chain_head(), Some(&HeaderHash::new(vec![1]).into()));
            buf.put_header(HeaderHash::new(vec![2]).into());
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(buf.chain_head(), Some(&HeaderHash::new(vec![2]).into()));
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(key, _)| key).collect();
            assert_eq!(items, vec![0, 1, 2]);
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.put_header(HeaderHash::new(vec![3]).into());
            buf.put_header(HeaderHash::new(vec![4]).into());
            buf.put_header(HeaderHash::new(vec![5]).into());
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(buf.chain_head(), Some(&HeaderHash::new(vec![5]).into()));
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(_, i)| i.tx_seq).collect();
            assert_eq!(items, vec![0, 0, 0, 1, 1, 1]);
            Ok(())
        })?;

        Ok(())
    }

    #[tokio::test]
    async fn chain_sequence_head_moved() -> anyhow::Result<()> {
        let arc1 = test_cell_env();
        let arc2 = arc1.clone();
        let (tx1, rx1) = tokio::sync::oneshot::channel();
        let (tx2, rx2) = tokio::sync::oneshot::channel();

        let task1 = tokio::spawn(async move {
            let env = arc1.guard().await;
            let dbs = arc1.clone().dbs().await?;
            let reader = env.reader()?;
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.put_header(HeaderHash::new(vec![0]).into());
            buf.put_header(HeaderHash::new(vec![1]).into());
            buf.put_header(HeaderHash::new(vec![2]).into());

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
            buf.put_header(HeaderHash::new(vec![3]).into());
            buf.put_header(HeaderHash::new(vec![4]).into());
            buf.put_header(HeaderHash::new(vec![5]).into());

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            tx2.send(()).unwrap();
            Result::<_, SourceChainError>::Ok(())
        });

        let (result1, result2) = tokio::join!(task1, task2);

        assert_eq!(
            result1.unwrap(),
            Err(SourceChainError::HeadMoved(
                None,
                Some(HeaderHash::new(vec![5]).into())
            ))
        );
        assert!(result2.unwrap().is_ok());

        Ok(())
    }
}
