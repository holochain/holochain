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
use holochain_state::{
    buffer::{BufferedStore, IntKvBuf},
    db::{GetDb, CHAIN_SEQUENCE},
    error::DatabaseResult,
    prelude::{Readable, Writer},
};
use holochain_types::composite_hash::HeaderAddress;
use serde::{Deserialize, Serialize};
use tracing::*;

/// A Value in the ChainSequence database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_address: HeaderAddress,
    tx_seq: u32,
    dht_transforms_complete: bool,
}

type Store<'env, R> = IntKvBuf<'env, u32, ChainSequenceItem, R>;

/// A BufferedStore for interacting with the ChainSequence database
pub struct ChainSequenceBuf<'env, R: Readable> {
    db: Store<'env, R>,
    next_index: u32,
    tx_seq: u32,
    current_head: Option<HeaderAddress>,
    persisted_head: Option<HeaderAddress>,
}

impl<'env, R: Readable> ChainSequenceBuf<'env, R> {
    /// Create a new instance from a read-only transaction and a database reference
    pub fn new(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let db: Store<'env, R> = IntKvBuf::new(reader, dbs.get_db(&*CHAIN_SEQUENCE)?)?;
        Self::from_db(db)
    }

    /// Create a new instance from a new read-only transaction, using the same database
    /// as an existing instance. Useful for getting a fresh read-only snapshot of a database.
    pub fn with_reader<RR: Readable>(
        &self,
        reader: &'env RR,
    ) -> DatabaseResult<ChainSequenceBuf<'env, RR>> {
        Self::from_db(self.db.with_reader(reader))
    }

    fn from_db<RR: Readable>(db: Store<'env, RR>) -> DatabaseResult<ChainSequenceBuf<'env, RR>> {
        let latest = db.iter_raw_reverse()?.next();
        debug!("{:?}", latest);
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

    /// Get the chain head, AKA top chain header. None if the chain is empty.
    pub fn chain_head(&self) -> Option<&HeaderAddress> {
        self.current_head.as_ref()
    }

    /// The length is just the next index
    pub fn len(&self) -> usize {
        self.next_index as usize
    }

    /// Add a header to the chain, setting all other values automatically.
    /// This is intentionally the only way to modify this database.
    #[instrument(skip(self))]
    pub fn put_header(&mut self, header_address: HeaderAddress) {
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
    use holo_hash::holo_hash_core::HeaderHash;
    use holochain_state::{
        env::{ReadManager, WriteManager},
        error::DatabaseResult,
        test_utils::test_cell_env,
    };
    use holochain_types::observability;

    #[tokio::test(threaded_scheduler)]
    async fn chain_sequence_scratch_awareness() -> DatabaseResult<()> {
        observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await;
        env.with_reader(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(buf.chain_head(), None);
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            );
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::new(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            );
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::new(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            );
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::new(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                    ])
                    .into()
                )
            );
            Ok(())
        })
    }

    #[tokio::test(threaded_scheduler)]
    async fn chain_sequence_functionality() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            );
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::new(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::new(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                    ])
                    .into()
                )
            );
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(key, _)| key).collect();
            assert_eq!(items, vec![0, 1, 2]);
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
                ])
                .into(),
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 4,
                ])
                .into(),
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
                ])
                .into(),
            );
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            Ok(())
        })?;

        env.with_reader::<SourceChainError, _, _>(|reader| {
            let buf = ChainSequenceBuf::new(&reader, &dbs)?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::new(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5
                    ])
                    .into()
                )
            );
            let items: Vec<u32> = buf.db.iter_raw()?.map(|(_, i)| i.tx_seq).collect();
            assert_eq!(items, vec![0, 0, 0, 1, 1, 1]);
            Ok(())
        })?;

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn chain_sequence_head_moved() -> anyhow::Result<()> {
        let arc1 = test_cell_env();
        let arc2 = arc1.clone();
        let (tx1, rx1) = tokio::sync::oneshot::channel();
        let (tx2, rx2) = tokio::sync::oneshot::channel();

        let task1 = tokio::spawn(async move {
            let env = arc1.guard().await;
            let dbs = arc1.dbs().await;
            let reader = env.reader()?;
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            );

            // let the other task run and make a commit to the chain head,
            // which will cause this one to error out when it re-enters and tries to commit
            tx1.send(()).unwrap();
            rx2.await.unwrap();

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        });

        let task2 = tokio::spawn(async move {
            rx1.await.unwrap();
            let env = arc2.guard().await;
            let dbs = arc2.dbs().await;
            let reader = env.reader()?;
            let mut buf = ChainSequenceBuf::new(&reader, &dbs)?;
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
                ])
                .into(),
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 4,
                ])
                .into(),
            );
            buf.put_header(
                HeaderHash::new(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
                ])
                .into(),
            );

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            tx2.send(()).unwrap();
            Result::<_, SourceChainError>::Ok(())
        });

        let (result1, result2) = tokio::join!(task1, task2);

        assert_eq!(
            result1.unwrap(),
            Err(SourceChainError::HeadMoved(
                None,
                Some(
                    HeaderHash::new(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5
                    ])
                    .into()
                )
            ))
        );
        assert!(result2.unwrap().is_ok());

        Ok(())
    }
}
