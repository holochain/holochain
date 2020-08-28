use super::error::WorkflowResult;
use crate::core::queue_consumer::{OneshotWriter, TriggerSender, WorkComplete};
use crate::core::state::{
    dht_op_integration::{AuthoredDhtOpsStore, AuthoredDhtOpsValue},
    source_chain::SourceChain,
    workspace::{Workspace, WorkspaceResult},
};
use holochain_state::{
    buffer::KvBufFresh,
    db::AUTHORED_DHT_OPS,
    prelude::{BufferedStore, EnvironmentRead, GetDb, Writer},
};
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
    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

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
            let (op, hash) = DhtOpHashed::from_content(op).into_inner();
            debug!(?hash, ?op);
            let value = AuthoredDhtOpsValue {
                op: op.to_light().await,
                receipt_count: 0,
                last_publish_time: None,
            };
            workspace.authored_dht_ops.put(hash, value)?;
        }
        // Mark the dht op as complete
        workspace.source_chain.complete_dht_op(index).await?;
    }

    Ok(WorkComplete::Complete)
}

pub struct ProduceDhtOpsWorkspace {
    pub source_chain: SourceChain,
    pub authored_dht_ops: AuthoredDhtOpsStore,
}

impl ProduceDhtOpsWorkspace {
    pub async fn new(env: EnvironmentRead, db: &impl GetDb) -> WorkspaceResult<Self> {
        let authored_dht_ops = db.get_db(&*AUTHORED_DHT_OPS)?;
        Ok(Self {
            source_chain: SourceChain::public_only(env.clone(), db).await?,
            authored_dht_ops: KvBufFresh::new(env.clone(), authored_dht_ops),
        })
    }
}

impl Workspace for ProduceDhtOpsWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(writer)?;
        self.authored_dht_ops.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::genesis_workflow::tests::fake_genesis;
    use super::*;
    use crate::core::state::source_chain::SourceChain;

    use ::fixt::prelude::*;
    use fallible_iterator::FallibleIterator;
    use holo_hash::*;

    use holochain_state::{
        env::{ReadManager, WriteManager},
        test_utils::test_cell_env,
    };
    use holochain_types::{
        dht_op::{produce_ops_from_element, DhtOp},
        fixt::*,
        observability, Entry, EntryHashed,
    };
    use holochain_zome_types::{
        entry_def::EntryVisibility,
        header::{builder, EntryType},
    };
    use matches::assert_matches;
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
            let (app_entry, entry_hash) = EntryHashed::from_content(app_entry).into();
            let app_entry_type = holochain_types::fixt::AppEntryTypeFixturator::new(visibility)
                .next()
                .unwrap();
            source_chain
                .put(
                    builder::EntryCreate {
                        entry_type: EntryType::App(app_entry_type),
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
            produce_ops_from_element(&element).await.unwrap()
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn elements_produce_ops() {
        observability::test_run().ok();
        let test_env = test_cell_env();
        let env = test_env.env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        // Setup the database and expected data
        let expected_hashes: HashSet<_> = {
            let mut td = TestData::new();
            let mut source_chain = SourceChain::new(env.clone().into(), &dbs).await.unwrap();

            // Add genesis so we can use the source chain
            fake_genesis(&mut source_chain).await.unwrap();
            let headers: Vec<_> = source_chain.iter_back().collect().unwrap();
            // The ops will be created from start to end of the chain
            let headers: Vec<_> = headers.into_iter().rev().collect();
            let mut all_ops = Vec::new();
            // Collect the ops from genesis
            for h in headers {
                let ops = produce_ops_from_element(
                    &source_chain
                        .get_element(h.as_hash())
                        .await
                        .unwrap()
                        .unwrap(),
                )
                .await
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
                .into_iter()
                .flatten()
                .map(|o| DhtOpHash::from_data(o))
                .collect()
        };

        // Run the workflow and commit it
        {
            let mut workspace = ProduceDhtOpsWorkspace::new(env.clone().into(), &dbs)
                .await
                .unwrap();
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
            let workspace = ProduceDhtOpsWorkspace::new(env.clone().into(), &dbs)
                .await
                .unwrap();

            // Get the authored ops
            let authored_results = workspace
                .authored_dht_ops
                .iter(&reader)
                .unwrap()
                .map(|(k, v)| {
                    assert_matches!(v, AuthoredDhtOpsValue {
                        receipt_count: 0,
                        last_publish_time: None,
                        ..
                    });

                    Ok(DhtOpHash::with_pre_hashed(k.to_vec()))
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
            let mut workspace = ProduceDhtOpsWorkspace::new(env.clone().into(), &dbs)
                .await
                .unwrap();
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
            let workspace = ProduceDhtOpsWorkspace::new(env.clone().into(), &dbs)
                .await
                .unwrap();
            let env_ref = env.guard().await;
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
