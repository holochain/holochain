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
use holochain_state::{
    buffer::{BufferedStore, KvIntBufFresh, KvIntStore},
    db::{GetDb, CHAIN_SEQUENCE},
    error::{DatabaseError, DatabaseResult},
    fresh_reader,
    prelude::*,
};
use serde::{Deserialize, Serialize};
use tokio_safe_block_on::tokio_safe_block_forever_on;
use tracing::*;

/// A Value in the ChainSequence database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainSequenceItem {
    header_address: HeaderHash,
    tx_seq: u32,
    dht_transforms_complete: bool,
}

type Store = KvIntBufFresh<ChainSequenceItem>;

/// A BufferedStore for interacting with the ChainSequence database
pub struct ChainSequenceBuf {
    buf: Store,
    next_index: u32,
    tx_seq: u32,
    current_head: Option<HeaderHash>,
    persisted_head: Option<HeaderHash>,
}

impl ChainSequenceBuf {
    /// Create a new instance
    pub async fn new(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let buf: Store = KvIntBufFresh::new(env.clone(), dbs.get_db(&*CHAIN_SEQUENCE)?);
        let (next_index, tx_seq, current_head) =
            fresh_reader!(env, |r| { Self::head_info(buf.store(), &r) })?;
        let persisted_head = current_head.clone();

        Ok(ChainSequenceBuf {
            buf,
            next_index,
            tx_seq,
            current_head,
            persisted_head,
        })
    }

    fn head_info<R: Readable>(
        store: &KvIntStore<ChainSequenceItem>,
        r: &R,
    ) -> DatabaseResult<(u32, u32, Option<HeaderHash>)> {
        let latest = store.iter(r)?.rev().next()?;
        debug!("{:?}", latest);
        DatabaseResult::Ok(
            latest
                .map(|(key, item)| {
                    (
                        // TODO: this is a bit ridiculous -- reevaluate whether the
                        //       IntKey is really needed (vs simple u32)
                        u32::from(IntKey::from_key_bytes_fallible(key)) + 1,
                        item.tx_seq + 1,
                        Some(item.header_address),
                    )
                })
                .unwrap_or((0, 0, None)),
        )
    }

    /// Get the chain head, AKA top chain header. None if the chain is empty.
    pub fn chain_head(&self) -> Option<&HeaderHash> {
        self.current_head.as_ref()
    }

    /// The length is just the next index
    pub fn len(&self) -> usize {
        self.next_index as usize
    }

    /// Get a header at an index
    pub async fn get(&self, i: u32) -> DatabaseResult<Option<HeaderHash>> {
        self.buf
            .get(&i.into())
            .await
            .map(|seq_item| seq_item.map(|si| si.header_address))
    }

    /// Add a header to the chain, setting all other values automatically.
    /// This is intentionally the only way to modify this database.
    #[instrument(skip(self))]
    pub fn put_header(&mut self, header_address: HeaderHash) -> DatabaseResult<()> {
        self.buf.put(
            self.next_index.into(),
            ChainSequenceItem {
                header_address: header_address.clone(),
                tx_seq: self.tx_seq,
                dht_transforms_complete: false,
            },
        )?;
        trace!(self.next_index);
        self.next_index += 1;
        self.current_head = Some(header_address);
        Ok(())
    }

    pub fn get_items_with_incomplete_dht_ops<'txn, R: Readable>(
        &self,
        r: &'txn R,
    ) -> SourceChainResult<
        Box<dyn FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError> + 'txn>,
    > {
        if !self.buf.is_scratch_fresh() {
            return Err(SourceChainError::ScratchNotFresh);
        }
        // TODO: PERF: Currently this checks every header but we could keep
        // a list of indices for only the headers which have been transformed.
        Ok(Box::new(self.buf.store().iter(r)?.filter_map(|(i, c)| {
            Ok(if !c.dht_transforms_complete {
                Some((IntKey::from_key_bytes_fallible(i).into(), c.header_address))
            } else {
                None
            })
        })))
    }

    pub async fn complete_dht_op(&mut self, i: u32) -> SourceChainResult<()> {
        if let Some(mut c) = self.buf.get(&i.into()).await? {
            c.dht_transforms_complete = true;
            self.buf.put(i.into(), c)?;
        }
        Ok(())
    }
}

impl BufferedStore for ChainSequenceBuf {
    type Error = SourceChainError;

    fn is_clean(&self) -> bool {
        self.buf.is_clean()
    }

    /// Commit to the source chain, performing an as-at check and returning a
    /// SourceChainError::HeadMoved error if the as-at check fails
    fn flush_to_txn(self, writer: &mut Writer) -> SourceChainResult<()> {
        if self.is_clean() {
            return Ok(());
        }
        let env = self.buf.env().clone();
        // FIXME: consider making the whole method async
        let db =
            tokio_safe_block_forever_on(async move { env.dbs().await.get_db(&*CHAIN_SEQUENCE) })?;
        let (_, _, persisted_head) = ChainSequenceBuf::head_info(&KvIntStore::new(db), writer)?;
        let (old, new) = (self.persisted_head, persisted_head);
        if old != new {
            Err(SourceChainError::HeadMoved(old, new))
        } else {
            Ok(self.buf.flush_to_txn(writer)?)
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::{BufferedStore, ChainSequenceBuf, SourceChainError};
    use crate::core::state::source_chain::SourceChainResult;
    use holo_hash::HeaderHash;
    use holochain_state::{
        env::{ReadManager, WriteManager},
        error::DatabaseResult,
        prelude::*,
        test_utils::test_cell_env,
    };
    use holochain_types::observability;
    use matches::assert_matches;

    #[tokio::test(threaded_scheduler)]
    async fn chain_sequence_scratch_awareness() -> DatabaseResult<()> {
        observability::test_run().ok();
        let arc = test_cell_env();
        let dbs = arc.dbs().await;
        {
            let mut buf = ChainSequenceBuf::new(arc.clone().into(), &dbs).await?;
            assert_eq!(buf.chain_head(), None);
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_bytes(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_bytes(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_bytes(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                    ])
                    .into()
                )
            );
            Ok(())
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn chain_sequence_functionality() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await;

        {
            let mut buf = ChainSequenceBuf::new(arc.clone().into(), &dbs).await?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_bytes(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            )?;
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }

        let reader = env.reader()?;
        {
            let buf = ChainSequenceBuf::new(arc.clone().into(), &dbs).await?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_bytes(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                    ])
                    .into()
                )
            );
            let items: Vec<u32> = buf
                .buf
                .store()
                .iter(&reader)?
                .map(|(key, _)| Ok(IntKey::from_key_bytes_fallible(key).into()))
                .collect()?;
            assert_eq!(items, vec![0, 1, 2]);
        }

        {
            let mut buf = ChainSequenceBuf::new(arc.clone().into(), &dbs).await?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 4,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
                ])
                .into(),
            )?;
            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }

        let reader = env.reader()?;
        {
            let buf = ChainSequenceBuf::new(arc.clone().into(), &dbs).await?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_bytes(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5
                    ])
                    .into()
                )
            );
            let items: Vec<u32> = buf
                .buf
                .store()
                .iter(&reader)?
                .map(|(_, i)| Ok(i.tx_seq))
                .collect()?;
            assert_eq!(items, vec![0, 0, 0, 1, 1, 1]);
        }

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
            let mut buf = ChainSequenceBuf::new(arc1.clone().into(), &dbs).await?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            )?;

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
            let mut buf = ChainSequenceBuf::new(arc2.clone().into(), &dbs).await?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 4,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_bytes(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
                ])
                .into(),
            )?;

            env.with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            tx2.send(()).unwrap();
            Result::<_, SourceChainError>::Ok(())
        });

        let (result1, result2) = tokio::join!(task1, task2);

        let expected_hash = HeaderHash::from_raw_bytes(vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 5,
        ])
        .into();
        assert_matches!(
            result1.unwrap(),
            Err(SourceChainError::HeadMoved(
                None,
                Some(
                    hash
                )
            ))
            if hash == expected_hash
        );
        assert!(result2.unwrap().is_ok());

        Ok(())
    }
}
