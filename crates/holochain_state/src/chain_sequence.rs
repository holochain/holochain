/// The BufferedStore for the Chain Sequence database
/// This database serves several purposes:
/// - enables fast forward iteration over the entire source chain
/// - knows what the chain head is, by accessing the last item's header address
/// - stores information about which headers were committed in the same transactional bundle
/// - stores info about whether each entry has undergone DHT op generation and publishing
///
/// When committing the ChainSequence db, a special step is taken to ensure source chain consistency.
/// If the chain head has moved since the db was created, committing the transaction fails with a special error type.
use crate::source_chain::{SourceChainError, SourceChainResult};
use fallible_iterator::DoubleEndedFallibleIterator;
use holo_hash::HeaderHash;
use holochain_sqlite::buffer::BufferedStore;
use holochain_sqlite::buffer::KvIntBufFresh;
use holochain_sqlite::buffer::KvIntStore;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::fresh_reader;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use serde::Deserialize;
use serde::Serialize;
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
    pub fn new(env: EnvRead) -> DatabaseResult<Self> {
        let buf: Store = KvIntBufFresh::new(env.clone(), env.get_table(TableName::ChainSequence)?);
        let (next_index, tx_seq, current_head) =
            fresh_reader!(env, |mut r| { Self::head_info(buf.store(), &mut r) })?;
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
        r: &mut R,
    ) -> DatabaseResult<(u32, u32, Option<HeaderHash>)> {
        let latest = store.iter(r)?.next_back()?;
        debug!("{:?}", latest);
        DatabaseResult::Ok(
            latest
                .map(|(key, item)| {
                    (
                        // TODO: this is a bit ridiculous -- reevaluate whether the
                        //       IntKey is really needed (vs simple u32)
                        u32::from(IntKey::from_key_bytes_or_friendly_panic(&key)) + 1,
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

    /// empty if len is 0
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The length is just the next index
    pub fn len(&self) -> usize {
        self.next_index as usize
    }

    /// Get a header at an index
    pub fn get(&self, i: u32) -> DatabaseResult<Option<HeaderHash>> {
        self.buf
            .get(&i.into())
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
        r: &'txn mut R,
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
                Some((
                    IntKey::from_key_bytes_or_friendly_panic(i.as_slice()).into(),
                    c.header_address,
                ))
            } else {
                None
            })
        })))
    }

    pub fn complete_dht_op(&mut self, i: u32) -> SourceChainResult<()> {
        if let Some(mut c) = self.buf.get(&i.into())? {
            c.dht_transforms_complete = true;
            self.buf.put(i.into(), c)?;
        }
        Ok(())
    }

    /// If this transaction hasn't moved the chain
    /// we don't need to check for as at on write.
    /// This helps avoid failed writes when nothing
    /// is actually being written by a workflow
    /// or when produce_dht_ops updates the
    /// dht_transforms_complete.
    pub fn chain_moved_in_this_transaction(&self) -> bool {
        self.current_head != self.persisted_head
    }
}

impl BufferedStore for ChainSequenceBuf {
    type Error = SourceChainError;

    fn is_clean(&self) -> bool {
        self.buf.is_clean()
    }

    /// Commit to the source chain, performing an as-at check and returning a
    /// SourceChainError::HeadMoved error if the as-at check fails
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> SourceChainResult<()> {
        // Nothing to write
        if self.is_clean() {
            return Ok(());
        }

        // Writing a chain move
        let env = self.buf.env().clone();
        let db = env.get_table(TableName::ChainSequence)?;
        let (_, _, persisted_head) = ChainSequenceBuf::head_info(&KvIntStore::new(db), writer)?;
        let persisted_head_moved = self.persisted_head != persisted_head;
        if persisted_head_moved && self.chain_moved_in_this_transaction() {
            Err(SourceChainError::HeadMoved(
                self.persisted_head.to_owned(),
                persisted_head,
            ))
        } else {
            Ok(self.buf.flush_to_txn_ref(writer)?)
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::BufferedStore;
    use super::ChainSequenceBuf;
    use super::SourceChainError;
    use crate::{source_chain::SourceChainResult, test_utils::test_cell_env};
    use holo_hash::HeaderHash;
    use holochain_sqlite::prelude::*;
    use matches::assert_matches;
    use observability;

    #[tokio::test(flavor = "multi_thread")]
    async fn chain_sequence_scratch_awareness() -> DatabaseResult<()> {
        observability::test_run().ok();
        let test_env = test_cell_env();
        let arc = test_env.env();
        {
            let mut buf = ChainSequenceBuf::new(arc.clone().into())?;
            assert_eq!(buf.chain_head(), None);
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_36(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_36(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_36(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                    ])
                    .into()
                )
            );
            Ok(())
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chain_sequence_functionality() -> SourceChainResult<()> {
        let test_env = test_cell_env();
        let arc = test_env.env();

        {
            let mut buf = ChainSequenceBuf::new(arc.clone().into())?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            )?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_36(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1
                    ])
                    .into()
                )
            );
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            )?;
            arc.conn()
                .unwrap()
                .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }
        let mut g = arc.conn().unwrap();
        g.with_reader(|mut reader| {
            let buf = ChainSequenceBuf::new(arc.clone().into())?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_36(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2
                    ])
                    .into()
                )
            );
            let items: Vec<u32> = buf
                .buf
                .store()
                .iter(&mut reader)?
                .map(|(key, _)| Ok(IntKey::from_key_bytes_or_friendly_panic(&key).into()))
                .collect()?;
            assert_eq!(items, vec![0, 1, 2]);
            DatabaseResult::Ok(())
        })?;

        {
            let mut buf = ChainSequenceBuf::new(arc.clone().into())?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 4,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
                ])
                .into(),
            )?;
            arc.conn()
                .unwrap()
                .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
        }
        let mut g = arc.conn().unwrap();
        g.with_reader(|mut reader| {
            let buf = ChainSequenceBuf::new(arc.clone().into())?;
            assert_eq!(
                buf.chain_head(),
                Some(
                    &HeaderHash::from_raw_36(vec![
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5
                    ])
                    .into()
                )
            );
            let items: Vec<u32> = buf
                .buf
                .store()
                .iter(&mut reader)?
                .map(|(_, i)| Ok(i.tx_seq))
                .collect()?;
            assert_eq!(items, vec![0, 0, 0, 1, 1, 1]);
            Ok(())
        })
    }

    /// If we attempt to move the chain head, but it has already moved from
    /// under us, error
    #[tokio::test(flavor = "multi_thread")]
    async fn chain_sequence_head_moved_triggers_error() -> anyhow::Result<()> {
        let test_env = test_cell_env();
        let arc1 = test_env.env();
        let arc2 = test_env.env();
        let (tx1, rx1) = tokio::sync::oneshot::channel();
        let (tx2, rx2) = tokio::sync::oneshot::channel();

        // Attempt to move the chain concurrently-- this one fails
        let task1 = tokio::spawn(async move {
            let mut buf = ChainSequenceBuf::new(arc1.clone().into())?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            )?;

            // let the other task run and make a commit to the chain head,
            // which will cause this one to error out when it re-enters and tries to commit
            tx1.send(()).unwrap();
            rx2.await.unwrap();

            arc1.conn()
                .unwrap()
                .with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        });

        // Attempt to move the chain concurrently -- this one succeeds
        let task2 = tokio::spawn(async move {
            rx1.await.unwrap();
            let mut buf = ChainSequenceBuf::new(arc2.clone().into())?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 4,
                ])
                .into(),
            )?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
                ])
                .into(),
            )?;

            arc2.conn()
                .unwrap()
                .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            tx2.send(()).unwrap();
            Result::<_, SourceChainError>::Ok(())
        });

        let (result1, result2) = tokio::join!(task1, task2);

        let expected_hash = HeaderHash::from_raw_36(vec![
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

    /// If the chain head has moved from under us, but we are not moving the
    /// chain head ourselves, proceed as usual
    #[tokio::test(flavor = "multi_thread")]
    async fn chain_sequence_head_moved_triggers_no_error_if_clean() -> anyhow::Result<()> {
        let test_env = test_cell_env();
        let arc1 = test_env.env();
        let arc2 = test_env.env();
        let (tx1, rx1) = tokio::sync::oneshot::channel();
        let (tx2, rx2) = tokio::sync::oneshot::channel();

        // Add a few things to start with
        let mut buf = ChainSequenceBuf::new(arc1.clone().into())?;
        buf.put_header(
            HeaderHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
            ])
            .into(),
        )?;
        buf.put_header(
            HeaderHash::from_raw_36(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 1,
            ])
            .into(),
        )?;
        arc1.conn()
            .unwrap()
            .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;

        // Modify the chain without adding a header -- this succeeds
        let task1 = tokio::spawn(async move {
            let mut buf = ChainSequenceBuf::new(arc1.clone().into())?;
            buf.complete_dht_op(0)?;

            // let the other task run and make a commit to the chain head,
            // to demonstrate the chain moving underneath us
            tx1.send(()).unwrap();
            rx2.await.unwrap();

            arc1.conn()
                .unwrap()
                .with_commit(|mut writer| buf.flush_to_txn(&mut writer))
        });

        // Add a header to the chain -- there is no collision, so this succeeds
        let task2 = tokio::spawn(async move {
            rx1.await.unwrap();
            let mut buf = ChainSequenceBuf::new(arc2.clone().into())?;
            buf.put_header(
                HeaderHash::from_raw_36(vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                ])
                .into(),
            )?;

            arc2.conn()
                .unwrap()
                .with_commit(|mut writer| buf.flush_to_txn(&mut writer))?;
            tx2.send(()).unwrap();
            Result::<_, SourceChainError>::Ok(())
        });

        let (result1, result2) = tokio::join!(task1, task2);

        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());

        Ok(())
    }
}
