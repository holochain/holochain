use super::error::WorkflowResult;
use crate::core::queue_consumer::OneshotWriter;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holochain_lmdb::buffer::KvBufFresh;
use holochain_lmdb::db::AUTHORED_DHT_OPS;
use holochain_lmdb::prelude::BufferedStore;
use holochain_lmdb::prelude::EnvironmentRead;
use holochain_lmdb::prelude::GetDb;
use holochain_lmdb::prelude::Writer;
use holochain_state::prelude::*;
use holochain_types::dht_op::DhtOpHashed;
use tracing::*;

pub mod dht_op_light;

#[instrument(skip(workspace, writer, trigger_publish))]
pub async fn produce_dht_ops_workflow(
    mut workspace: ProduceDhtOpsWorkspace,
    writer: OneshotWriter,
    trigger_publish: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    let complete = produce_dht_ops_workflow_inner(&mut workspace).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer.with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))?;

    // trigger other workflows
    trigger_publish.trigger();

    Ok(complete)
}

async fn produce_dht_ops_workflow_inner(
    workspace: &mut ProduceDhtOpsWorkspace,
) -> WorkflowResult<WorkComplete> {
    debug!("Starting dht op workflow");
    let all_ops = workspace.source_chain.get_incomplete_dht_ops().await?;

    for (index, ops) in all_ops {
        for op in ops {
            let (op, hash) = DhtOpHashed::from_content_sync(op).into_inner();
            debug!(?hash, ?op);
            let value = AuthoredDhtOpsValue {
                op: op.to_light(),
                receipt_count: 0,
                last_publish_time: None,
            };
            workspace.authored_dht_ops.put(hash, value)?;
        }
        // Mark the dht op as complete
        workspace.source_chain.complete_dht_op(index)?;
    }

    Ok(WorkComplete::Complete)
}

pub struct ProduceDhtOpsWorkspace {
    pub source_chain: SourceChain,
    pub authored_dht_ops: AuthoredDhtOpsStore,
}

impl ProduceDhtOpsWorkspace {
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let authored_dht_ops = env.get_db(&*AUTHORED_DHT_OPS)?;
        Ok(Self {
            source_chain: SourceChain::public_only(env.clone())?,
            authored_dht_ops: KvBufFresh::new(env, authored_dht_ops),
        })
    }
}

impl Workspace for ProduceDhtOpsWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn_ref(writer)?;
        self.authored_dht_ops.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::genesis_workflow::tests::fake_genesis;
    use super::*;
    use holochain_state::source_chain::SourceChain;

    use ::fixt::prelude::*;
    use fallible_iterator::FallibleIterator;
    use holo_hash::*;

    use holochain_lmdb::env::ReadManager;
    use holochain_lmdb::env::WriteManager;
    use holochain_lmdb::test_utils::test_cell_env;
    use holochain_types::dht_op::produce_ops_from_element;
    use holochain_types::dht_op::DhtOp;
    use holochain_types::fixt::*;
    use holochain_types::EntryHashed;
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::header::builder;
    use holochain_zome_types::header::EntryType;
    use holochain_zome_types::Entry;
    use matches::assert_matches;
    use observability;
    use std::collections::HashSet;

    struct TestData {
        app_entry: Box<dyn Iterator<Item = Entry>>,
    }

    impl TestData {
        fn new() -> Self {
            let app_entry =
                Box::new(AppEntryBytesFixturator::new(Unpredictable).map(|b| Entry::App(b)));
            Self { app_entry }
        }

        async fn put_fix_entry(
            &mut self,
            source_chain: &mut SourceChain,
            visibility: EntryVisibility,
        ) -> Vec<DhtOp> {
            let app_entry = self.app_entry.next().unwrap();
            let (app_entry, entry_hash) = EntryHashed::from_content_sync(app_entry).into();
            let app_entry_type = holochain_types::fixt::AppEntryTypeFixturator::new(visibility)
                .next()
                .unwrap();
            source_chain
                .put(
                    builder::Create {
                        entry_type: EntryType::App(app_entry_type),
                        entry_hash,
                    },
                    Some(app_entry),
                    None,
                )
                .await
                .unwrap();
            let element = source_chain
                .get_element(source_chain.chain_head().unwrap())
                .unwrap()
                .unwrap();
            produce_ops_from_element(&element).unwrap()
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn elements_produce_ops() {
        observability::test_run().ok();
        let test_env = test_cell_env();
        let env = test_env.env();
        let env_ref = env.guard();

        // Setup the database and expected data
        let expected_hashes: HashSet<_> = {
            let mut td = TestData::new();
            let mut source_chain = SourceChain::new(env.clone().into()).unwrap();

            // Add genesis so we can use the source chain
            fake_genesis(&mut source_chain).await.unwrap();
            let headers: Vec<_> = source_chain.iter_back().collect().unwrap();
            // The ops will be created from start to end of the chain
            let headers: Vec<_> = headers.into_iter().rev().collect();
            let mut all_ops = Vec::new();
            // Collect the ops from genesis
            for h in headers {
                let ops = produce_ops_from_element(
                    &source_chain.get_element(h.as_hash()).unwrap().unwrap(),
                )
                .unwrap();
                all_ops.push(ops);
            }

            // Add some entries and collect the expected ops
            for _ in 0..10 as u8 {
                all_ops.push(
                    td.put_fix_entry(&mut source_chain, EntryVisibility::Public)
                        .await,
                );
                all_ops.push(
                    td.put_fix_entry(&mut source_chain, EntryVisibility::Private)
                        .await
                        .into_iter()
                        // Remove the StoreEntry from private entries
                        .filter(|op| {
                            if let DhtOp::StoreEntry(_, _, _) = op {
                                false
                            } else {
                                true
                            }
                        })
                        .collect(),
                );
            }

            env_ref
                .with_commit(|writer| source_chain.flush_to_txn(writer))
                .unwrap();

            all_ops
                .iter()
                .flatten()
                .map(|o| DhtOpHash::with_data_sync(o))
                .collect()
        };

        // Run the workflow and commit it
        {
            let mut workspace = ProduceDhtOpsWorkspace::new(env.clone().into()).unwrap();
            let complete = produce_dht_ops_workflow_inner(&mut workspace)
                .await
                .unwrap();
            assert_matches!(complete, WorkComplete::Complete);
            env_ref
                .with_commit(|writer| workspace.flush_to_txn(writer))
                .unwrap();
        }

        // Pull out the results and check them
        let last_count = {
            let reader = env_ref.reader().unwrap();
            let workspace = ProduceDhtOpsWorkspace::new(env.clone().into()).unwrap();

            // Get the authored ops
            let authored_results = workspace
                .authored_dht_ops
                .iter(&reader)
                .unwrap()
                .map(|(k, v)| {
                    assert_matches!(
                        v,
                        AuthoredDhtOpsValue {
                            receipt_count: 0,
                            last_publish_time: None,
                            ..
                        }
                    );

                    Ok(DhtOpHash::from_raw_39_panicky(k.to_vec()))
                })
                .collect::<HashSet<_>>()
                .unwrap();
            for a in &authored_results {
                assert!(expected_hashes.contains(a), "{:?}", a);
            }

            // Check we got all the hashes
            assert_eq!(authored_results, expected_hashes);

            authored_results.len()
        };

        // Call the workflow again now the queue should be the same length as last time
        // because no new ops should hav been added
        {
            let mut workspace = ProduceDhtOpsWorkspace::new(env.clone().into()).unwrap();
            let complete = produce_dht_ops_workflow_inner(&mut workspace)
                .await
                .unwrap();
            assert_matches!(complete, WorkComplete::Complete);
            env_ref
                .with_commit(|writer| workspace.flush_to_txn(writer))
                .unwrap();
        }

        // Check the lengths are unchanged
        {
            let workspace = ProduceDhtOpsWorkspace::new(env.clone().into()).unwrap();
            let env_ref = env.guard();
            let reader = env_ref.reader().unwrap();
            let authored_count = workspace
                .authored_dht_ops
                .iter(&reader)
                .unwrap()
                .count()
                .unwrap();

            assert_eq!(last_count, authored_count);
        }
    }
}
