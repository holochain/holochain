use super::{error::WorkflowResult, InvokeZomeWorkspace, Workflow, WorkflowEffects};
use crate::core::state::workspace::{Workspace, WorkspaceError, WorkspaceResult};
use dht_op::{dht_op_to_light_basis, DhtOpLight};
use futures::FutureExt;
use holo_hash::DhtOpHash;
use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError};
use holochain_state::{
    buffer::KvBuf,
    db::{AUTHORED_DHT_OPS, INTEGRATION_QUEUE},
    prelude::{BufferedStore, GetDb, Reader, Writer},
};
use holochain_types::{
    composite_hash::AnyDhtHash, dht_op::DhtOpHashed, validate::ValidationStatus, Timestamp,
};
use must_future::MustBoxFuture;
use std::convert::TryFrom;
use tracing::*;
use tracing_futures::Instrument;

pub mod dht_op;

pub(crate) struct ProduceDhtOpWorkflow {}

impl<'env> Workflow<'env> for ProduceDhtOpWorkflow {
    type Output = ();
    type Workspace = ProduceDhtOpWorkspace<'env>;
    type Triggers = ();

    fn workflow(
        self,
        mut workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self>> {
        async {
            debug!("Starting dht op workflow");
            let invoke_zome_workspace = &mut workspace.invoke_zome_workspace;
            let all_ops = invoke_zome_workspace
                .source_chain
                .get_incomplete_dht_ops()
                .await?;
            for (index, ops) in all_ops {
                for op in ops {
                    let (op, hash) = DhtOpHashed::with_data(op).await.into();
                    debug!(?hash);
                    let cascade = invoke_zome_workspace.cascade();
                    // put the op in (using the "light hash form")
                    let (op, basis) = dht_op_to_light_basis(op, cascade).await?;
                    workspace.integration_queue.put(
                        (Timestamp::now(), hash.clone()).try_into()?,
                        IntegrationValue {
                            validation_status: ValidationStatus::Valid,
                            op,
                            basis,
                        },
                    )?;
                    workspace.authored_dht_ops.put(hash, 0)?;
                }
                // Mark the dht op as complete
                invoke_zome_workspace.source_chain.complete_dht_op(index)?;
            }
            // TODO: B-01567: Trigger IntegrateDhtOps workflow
            let fx = WorkflowEffects {
                workspace,
                callbacks: Default::default(),
                signals: Default::default(),
                triggers: Default::default(),
            };

            Ok(((), fx))
        }
        .instrument(trace_span!("ProduceDhtOpWorkflow"))
        .boxed()
        .into()
    }
}

#[derive(Hash, Eq, PartialEq)]
pub struct IntegrationQueueKey(SerializedBytes);
#[derive(serde::Deserialize, serde::Serialize, SerializedBytes)]
struct T(Timestamp, DhtOpHash);

impl AsRef<[u8]> for IntegrationQueueKey {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes().as_ref()
    }
}

impl TryFrom<(Timestamp, DhtOpHash)> for IntegrationQueueKey {
    type Error = SerializedBytesError;
    fn try_from(t: (Timestamp, DhtOpHash)) -> Result<Self, Self::Error> {
        Ok(Self(SerializedBytes::try_from(T(t.0, t.1))?))
    }
}

impl TryFrom<IntegrationQueueKey> for (Timestamp, DhtOpHash) {
    type Error = SerializedBytesError;
    fn try_from(key: IntegrationQueueKey) -> Result<Self, Self::Error> {
        let t = T::try_from(key.0)?;
        Ok((t.0, t.1))
    }
}

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrationValue {
    /// Thi ops validation status
    pub validation_status: ValidationStatus,
    /// Where to send this op
    pub basis: AnyDhtHash,
    /// Signatures and hashes of the op
    pub op: DhtOpLight,
}

pub(crate) struct ProduceDhtOpWorkspace<'env> {
    pub invoke_zome_workspace: InvokeZomeWorkspace<'env>,
    pub authored_dht_ops: KvBuf<'env, DhtOpHash, u32, Reader<'env>>,
    pub integration_queue: KvBuf<'env, IntegrationQueueKey, IntegrationValue, Reader<'env>>,
}

impl<'env> ProduceDhtOpWorkspace<'env> {
    // FIXME: Remove when used
    #[allow(dead_code)]
    pub(crate) fn new(reader: &'env Reader<'env>, db: &impl GetDb) -> WorkspaceResult<Self> {
        let authored_dht_ops = db.get_db(&*AUTHORED_DHT_OPS)?;
        let integration_queue = db.get_db(&*INTEGRATION_QUEUE)?;
        Ok(Self {
            invoke_zome_workspace: InvokeZomeWorkspace::new(reader, db)?,
            authored_dht_ops: KvBuf::new(reader, authored_dht_ops)?,
            integration_queue: KvBuf::new(reader, integration_queue)?,
        })
    }
}

impl<'env> Workspace<'env> for ProduceDhtOpWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.flush_to_txn(&mut writer)?;
        Ok(writer.commit()?)
    }
}

impl<'env> BufferedStore<'env> for ProduceDhtOpWorkspace<'env> {
    type Error = WorkspaceError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> Result<(), Self::Error> {
        self.invoke_zome_workspace.flush_to_txn(writer)?;
        self.authored_dht_ops.flush_to_txn(writer)?;
        self.integration_queue.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::genesis_workflow::tests::fake_genesis;
    use super::*;
    use crate::core::state::{chain_cas::ChainCasBuf, source_chain::SourceChain};

    use fallible_iterator::FallibleIterator;
    use fixt::prelude::*;
    use holo_hash::{Hashable, Hashed, HoloHashBaseExt};

    use holochain_state::{
        env::{ReadManager, WriteManager},
        test_utils::test_cell_env,
    };
    use holochain_types::{
        dht_op::{ops_from_element, DhtOp, DhtOpHashed},
        header::{builder, EntryType, NewEntryHeader},
        observability,
        test_utils::fake_app_entry_type,
        Entry, EntryHashed, Header,
    };
    use holochain_zome_types::entry_def::EntryVisibility;
    use matches::assert_matches;
    use std::collections::HashSet;
    use unwrap_to::unwrap_to;

    struct TestData {
        app_entry: Box<dyn Iterator<Item = Entry>>,
    }

    impl TestData {
        fn new() -> Self {
            let app_entry =
                Box::new(SerializedBytesFixturator::new(Unpredictable).map(|b| Entry::App(b)));
            Self { app_entry }
        }

        async fn put_fix_entry(
            &mut self,
            source_chain: &mut SourceChain<'_>,
            visibility: EntryVisibility,
        ) -> Vec<DhtOp> {
            let app_entry = self.app_entry.next().unwrap();
            let (app_entry, entry_hash) = EntryHashed::with_data(app_entry).await.unwrap().into();
            source_chain
                .put(
                    builder::EntryCreate {
                        entry_type: EntryType::App(fake_app_entry_type(0, visibility)),
                        entry_hash,
                    },
                    Some(app_entry),
                )
                .await
                .unwrap();
            let element = source_chain
                .get_element(source_chain.chain_head().unwrap())
                .await
                .unwrap()
                .unwrap();
            ops_from_element(&element).unwrap()
        }
    }

    #[instrument(skip(light, cas))]
    async fn light_to_op<'env>(light: DhtOpLight, cas: &ChainCasBuf<'env>) -> DhtOp {
        trace!(?light);
        match light {
            DhtOpLight::StoreElement(s, h, _) => {
                let e = cas.get_element(&h).await.unwrap().unwrap();
                let h = e.header().clone();
                let e = e.entry().as_option().map(|e| Box::new(e.clone()));
                DhtOp::StoreElement(s, h, e)
            }
            DhtOpLight::StoreEntry(s, h, _) => {
                let e = cas.get_element(&h).await.unwrap().unwrap();
                let h = match e.header().clone() {
                    Header::EntryCreate(c) => NewEntryHeader::Create(c),
                    Header::EntryUpdate(c) => NewEntryHeader::Update(c),
                    _ => panic!("Must be a header that creates an entry"),
                };
                let e = e.entry().as_option().map(|e| Box::new(e.clone())).unwrap();
                DhtOp::StoreEntry(s, h, e)
            }
            DhtOpLight::RegisterAgentActivity(s, h) => {
                let e = cas.get_header(&h).await.unwrap().unwrap();
                let h = e.header().clone();
                DhtOp::RegisterAgentActivity(s, h)
            }
            DhtOpLight::RegisterReplacedBy(s, h, _) => {
                let e = cas.get_element(&h).await.unwrap().unwrap();
                let h = unwrap_to!(e.header() => Header::EntryUpdate).clone();
                let e = e.entry().as_option().map(|e| Box::new(e.clone())).unwrap();
                DhtOp::RegisterReplacedBy(s, h, Some(e))
            }
            DhtOpLight::RegisterDeletedBy(s, h) => {
                let e = cas.get_header(&h).await.unwrap().unwrap();
                let h = unwrap_to!(e.header() => Header::EntryDelete).clone();
                DhtOp::RegisterDeletedBy(s, h)
            }
            DhtOpLight::RegisterAddLink(s, h) => {
                let e = cas.get_header(&h).await.unwrap().unwrap();
                let h = unwrap_to!(e.header() => Header::LinkAdd).clone();
                DhtOp::RegisterAddLink(s, h)
            }
            DhtOpLight::RegisterRemoveLink(s, h) => {
                let e = cas.get_header(&h).await.unwrap().unwrap();
                let h = unwrap_to!(e.header() => Header::LinkRemove).clone();
                DhtOp::RegisterRemoveLink(s, h)
            }
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn elements_produce_ops() {
        observability::test_run().ok();
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        // Setup the database and expected data
        let expected = {
            let reader = env_ref.reader().unwrap();
            let mut td = TestData::new();
            let mut source_chain = ProduceDhtOpWorkspace::new(&reader, &dbs)
                .unwrap()
                .invoke_zome_workspace
                .source_chain;

            // Add genesis so we can use the source chain
            fake_genesis(&mut source_chain).await.unwrap();
            let headers: Vec<_> = source_chain.iter_back().collect().unwrap();
            // The ops will be created from start to end of the chain
            let headers: Vec<_> = headers.into_iter().rev().collect();
            let mut all_ops = Vec::new();
            // Collect the ops from genesis
            for h in headers {
                let ops = ops_from_element(
                    &source_chain
                        .get_element(h.as_hash())
                        .await
                        .unwrap()
                        .unwrap(),
                )
                .unwrap();
                all_ops.push(ops);
            }

            // Add some entires and collect the expected ops
            for _ in 0..10 {
                all_ops.push(
                    td.put_fix_entry(&mut source_chain, EntryVisibility::Public)
                        .await,
                );
                all_ops.push(
                    td.put_fix_entry(&mut source_chain, EntryVisibility::Private)
                        .await,
                );
            }

            env_ref
                .with_commit(|writer| source_chain.flush_to_txn(writer))
                .unwrap();

            // Turn all the ops into hashes
            let mut expected = Vec::new();
            for ops in all_ops {
                for op in ops {
                    let (_, hash) = DhtOpHashed::with_data(op).await.into();
                    expected.push(hash);
                }
            }
            expected
        };

        // Run the workflow and commit it
        {
            let reader = env_ref.reader().unwrap();
            let workspace = ProduceDhtOpWorkspace::new(&reader, &dbs).unwrap();
            let workflow = ProduceDhtOpWorkflow {};
            let (_, effects) = workflow.workflow(workspace).await.unwrap();
            let writer = env_ref.writer_unmanaged().unwrap();
            effects.workspace.commit_txn(writer).unwrap();
        }

        // Pull out the results and check them
        let last_count = {
            let reader = env_ref.reader().unwrap();
            let workspace = ProduceDhtOpWorkspace::new(&reader, &dbs).unwrap();
            let mut times = Vec::new();
            let results = workspace
                .integration_queue
                .iter()
                .unwrap()
                .map(|(k, v)| {
                    let s = debug_span!("times");
                    let _g = s.enter();
                    let s = SerializedBytes::from(UnsafeBytes::from(k.to_vec()));
                    let t = T::try_from(s).unwrap();
                    debug!(time = ?t.0);
                    debug!(hash = ?t.1);
                    times.push(t.0);
                    // Check the status is Valid
                    assert_matches!(v.validation_status, ValidationStatus::Valid);
                    Ok(v.op)
                })
                .collect::<Vec<_>>()
                .unwrap();

            // Check that the integration queue is ordered by time
            times.into_iter().fold(None, |last, time| {
                if let Some(lt) = last {
                    // Check they are ordered by time
                    assert!(lt <= time);
                }
                Some(time)
            });

            // Get the authored ops
            let mut authored_results = workspace
                .authored_dht_ops
                .iter()
                .unwrap()
                .map(|(k, v)| {
                    assert_eq!(v, 0);
                    Ok(DhtOpHash::with_pre_hashed(k.to_vec()))
                })
                .collect::<Vec<_>>()
                .unwrap();

            // Convert the results to light ops (hashes only)
            let mut r = Vec::with_capacity(results.len());
            for light_op in results {
                let op =
                    light_to_op(light_op, workspace.invoke_zome_workspace.source_chain.cas()).await;
                let (_, hash) = DhtOpHashed::with_data(op).await.into();
                r.push(hash);
            }
            let r_h: HashSet<_> = r.iter().cloned().collect();
            let e_h: HashSet<_> = expected.iter().cloned().collect();
            let diff: HashSet<_> = r_h.difference(&e_h).collect();

            // Check for differences (redundant but useful for debugging)
            assert_eq!(diff, HashSet::new());

            // Check we got all the hashes
            assert_eq!(r, expected);

            // authored are in a different order so need to sort
            r.sort();
            authored_results.sort();
            // Check authored are all there
            assert_eq!(r, authored_results);
            r.len()
        };

        // Call the workflow again now the queue should be the same length as last time
        // because no new ops should hav been added
        {
            let reader = env_ref.reader().unwrap();
            let workspace = ProduceDhtOpWorkspace::new(&reader, &dbs).unwrap();
            let workflow = ProduceDhtOpWorkflow {};
            let (_, effects) = workflow.workflow(workspace).await.unwrap();
            let writer = env_ref.writer_unmanaged().unwrap();
            effects.workspace.commit_txn(writer).unwrap();
        }

        // Check the lengths are unchanged
        {
            let reader = env_ref.reader().unwrap();
            let workspace = ProduceDhtOpWorkspace::new(&reader, &dbs).unwrap();
            let count = workspace.integration_queue.iter().unwrap().count().unwrap();
            let authored_count = workspace.authored_dht_ops.iter().unwrap().count().unwrap();

            assert_eq!(last_count, count);
            assert_eq!(last_count, authored_count);
        }
    }
}
