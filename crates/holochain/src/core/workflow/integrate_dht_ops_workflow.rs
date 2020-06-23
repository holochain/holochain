//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        cascade::Cascade,
        chain_cas::ChainCasBuf,
        dht_op_integration::{IntegratedDhtOpsStore, IntegrationQueueStore, IntegrationValue},
        metadata::{MetadataBuf, MetadataBufT},
        workspace::{Workspace, WorkspaceResult},
    },
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::Hashed;
use holochain_state::{
    buffer::BufferedStore,
    buffer::KvBuf,
    db::{INTEGRATED_DHT_OPS, INTEGRATION_QUEUE},
    prelude::{GetDb, Reader, Writer},
};
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed},
    header::UpdateBasis,
    Header,
};
use produce_dht_ops_workflow::dht_op::DhtOpLight;
use tracing::*;

pub async fn integrate_dht_ops_workflow(
    mut workspace: IntegrateDhtOpsWorkspace<'_>,
    writer: OneshotWriter,
    trigger_publish: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    let result = integrate_dht_ops_workflow_inner(&mut workspace).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    // TODO: only trigger if we have integrated ops that we have authored them
    trigger_publish.trigger();

    Ok(result)
}

async fn integrate_dht_ops_workflow_inner(
    workspace: &mut IntegrateDhtOpsWorkspace<'_>,
) -> WorkflowResult<WorkComplete> {
    // Pull ops out of queue
    // TODO: PERF: Not collect, iterator cannot cross awaits
    // Find a way to do this.
    let ops = workspace
        .integration_queue
        .drain_iter_reverse()?
        .collect::<Vec<_>>()?;

    for value in ops {
        // TODO: Process each op
        let IntegrationValue { op, .. } = value.clone();
        let op_hash = match op {
            DhtOpLight::StoreElement(_, header, _) => {
                let signed_header = workspace
                    .cascade()
                    .dht_get_header_raw(&header)
                    .await
                    // TODO: handle error
                    .unwrap()
                    // TODO: handle header not found
                    .unwrap();
                let maybe_entry = match signed_header.header().entry_data() {
                    Some((hash, _)) => Some(
                        workspace
                            .cascade()
                            .dht_get_entry_raw(hash)
                            .await
                            // TODO: handle error
                            .unwrap()
                            // TODO: handle entry not found
                            .unwrap(),
                    ),
                    None => None,
                };
                // TODO: Put header into metadata
                match signed_header.header().clone() {
                    Header::LinkAdd(h) => workspace.meta.add_link(h).await?,
                    Header::LinkRemove(link_remove) => {
                        let link_add = workspace
                            .cascade()
                            .dht_get_header_raw(&link_remove.link_add_address)
                            .await
                            // TODO: Handle error
                            .unwrap()
                            // TODO: Handle link add missing
                            .unwrap()
                            .into_header_and_signature()
                            .0
                            .into_content();
                        let link_add = match link_add {
                            Header::LinkAdd(la) => la,
                            _ => panic!("Must be a link add"),
                        };

                        // Remove the link
                        workspace.meta.remove_link(
                            link_remove,
                            &link_add.base_address,
                            link_add.zome_id,
                            link_add.tag,
                        )?;
                    }
                    Header::EntryCreate(entry_create) => {
                        workspace.meta.add_create(entry_create).await?
                    }
                    Header::EntryUpdate(entry_update) => {
                        let entry = match entry_update.update_basis {
                            UpdateBasis::Header => None,
                            UpdateBasis::Entry => Some(
                                workspace
                                    .cascade()
                                    .dht_get_header_raw(&entry_update.replaces_address)
                                    .await?
                                    // TODO: Handle original header not found
                                    .unwrap()
                                    .header()
                                    .entry_data()
                                    // TODO: Handle update basis on entry but header has no entry
                                    .unwrap()
                                    .0
                                    .clone(),
                            ),
                        };
                        workspace.meta.add_update(entry_update, entry).await?;
                    }
                    Header::EntryDelete(entry_delete) => {
                        workspace.meta.add_delete(entry_delete).await?
                    }
                    _ => {}
                }
                // TODO: If we want to avoid these clones we could get the op hash from
                // the db but is it ok to trust an op hash from a db?
                workspace
                    .cas
                    .put(signed_header.clone(), maybe_entry.clone())?;

                let (header, sig) = signed_header.into_header_and_signature();
                DhtOpHashed::with_data(DhtOp::StoreElement(
                    sig,
                    header.into_content(),
                    maybe_entry.map(Hashed::into_content).map(Box::new),
                ))
                .await
                .into_hash()
            }
            DhtOpLight::StoreEntry(_, _, _) => todo!(),
            DhtOpLight::RegisterAgentActivity(_, _) => todo!(),
            DhtOpLight::RegisterReplacedBy(_, _, _) => todo!(),
            DhtOpLight::RegisterDeletedBy(_, _) => todo!(),
            DhtOpLight::RegisterAddLink(_, _) => todo!(),
            DhtOpLight::RegisterRemoveLink(_, _) => todo!(),
        };
        workspace.integrated_dht_ops.put(op_hash, value)?;
    }

    Ok(WorkComplete::Complete)
}

pub struct IntegrateDhtOpsWorkspace<'env> {
    // integration queue
    integration_queue: IntegrationQueueStore<'env>,
    // integrated ops
    integrated_dht_ops: IntegratedDhtOpsStore<'env>,
    // Cas for storing
    cas: ChainCasBuf<'env>,
    // metadata store
    meta: MetadataBuf<'env>,
    // cache for looking up entries
    cache: ChainCasBuf<'env>,
    // cached meta for the cascade
    cache_meta: MetadataBuf<'env>,
}

impl<'env> IntegrateDhtOpsWorkspace<'env> {
    pub fn cascade(&self) -> Cascade {
        Cascade::new(&self.cas, &self.meta, &self.cache, &self.cache_meta)
    }
}

impl<'env> Workspace<'env> for IntegrateDhtOpsWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBuf::new(reader, db)?;

        let db = dbs.get_db(&*INTEGRATION_QUEUE)?;
        let integration_queue = KvBuf::new(reader, db)?;

        let cas = ChainCasBuf::primary(reader, dbs, true)?;
        let cache = ChainCasBuf::cache(reader, dbs)?;
        let meta = MetadataBuf::primary(reader, dbs)?;
        let cache_meta = MetadataBuf::cache(reader, dbs)?;

        Ok(Self {
            integration_queue,
            integrated_dht_ops,
            cas,
            meta,
            cache,
            cache_meta,
        })
    }
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        // TODO: flush cas
        self.cas.flush_to_txn(writer)?;
        // TODO: flush metadata store
        // TODO: flush integrated
        warn!("unimplemented");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        core::{
            state::{
                cascade::{test_dbs_and_mocks, Cascade},
                dht_op_integration::IntegrationValue,
            },
            workflow::produce_dht_ops_workflow::dht_op::{dht_op_to_light_basis, DhtOpLight},
        },
        fixt::{EntryCreateFixturator, EntryFixturator, EntryUpdateFixturator, LinkAddFixturator},
    };
    use fixt::prelude::*;
    use holo_hash::{AgentPubKeyFixturator, DnaHashFixturator, Hashable, Hashed};
    use holochain_state::{
        buffer::BufferedStore,
        env::{EnvironmentRefRw, ReadManager, WriteManager},
        error::DatabaseError,
        test_utils::test_cell_env,
    };
    use holochain_types::{
        dht_op::{DhtOp, DhtOpHashed},
        fixt::{AppEntryTypeFixturator, SignatureFixturator},
        header::NewEntryHeader,
        observability,
        validate::ValidationStatus,
        EntryHashed, Timestamp,
    };
    use std::convert::TryInto;

    #[tokio::test(threaded_scheduler)]
    async fn test_store_entry() {
        // Create test env
        observability::test_run().ok();
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        // Setup test data
        let mut entry_create = fixt!(EntryCreate);
        let entry = fixt!(Entry);
        let entry_hash = EntryHashed::with_data(entry.clone())
            .await
            .unwrap()
            .into_hash();
        entry_create.entry_hash = entry_hash.clone();

        // create store entry
        let store_entry = DhtOp::StoreEntry(
            fixt!(Signature),
            NewEntryHeader::Create(entry_create.clone()),
            Box::new(entry.clone()),
        );

        // Create integration value
        let val = {
            let reader = env_ref.reader().unwrap();
            let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
            let cascade = workspace.cascade();

            let (op, basis) = dht_op_to_light_basis(store_entry.clone(), &cascade)
                .await
                .unwrap();
            IntegrationValue {
                validation_status: ValidationStatus::Valid,
                op,
                basis,
            }
        };

        // Add to integration queue
        {
            let reader = env_ref.reader().unwrap();
            let mut workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
            let op_hash = DhtOpHashed::with_data(store_entry.clone())
                .await
                .into_hash();

            workspace
                .integration_queue
                .put((Timestamp::now(), op_hash.clone()).try_into().unwrap(), val)
                .unwrap();

            env_ref
                .with_commit::<DatabaseError, _, _>(|writer| {
                    workspace.integration_queue.flush_to_txn(writer)?;
                    Ok(())
                })
                .unwrap();
        }

        // TODO: Add data to cache?

        // Call workflow
        {
            let reader = env_ref.reader().unwrap();
            let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
            let (mut qt, _rx) = TriggerSender::new();
            integrate_dht_ops_workflow(workspace, env.clone().into(), &mut qt)
                .await
                .unwrap();
        }

        // Check the entry is now in the Cas
        {
            let reader = env_ref.reader().unwrap();
            let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
            workspace
                .cas
                .get_entry(&entry_hash)
                .await
                .unwrap()
                .expect("Entry is not in cas");
        }
    }

    // Entries, Private Entries & Headers are stored to CAS
    #[tokio::test(threaded_scheduler)]
    async fn test_cas_update() {
        // Pre state
        // TODO: Entry A
        // TODO: Header A: EntryCreate creates Entry A
        // TODO: DhtOp A: StoreElement with Header A and Entry A
        // TODO: Integration Queue has Op A
        // TODO: Cache has Entry A and Header A
        // Test
        // TODO: Run workflow
        // Post state
        // TODO: Check Cas has Entry A and Header A
        // TODO: Check DhtOp A is in integrated ops db
        // TODO: Check metadata has Header A on Entry A

        // More general
        // For all DhtOp (private and public):
        // Put associated data into cache
        // Add DhtOps to integration queue
        // Run workflow
        // Check all headers from ops are in Cas
        // If the Op has an entry check it's in the Cas
        // Check all ops are in integrated ops db
        // If Op has an entry reference it to the header in the metadata
    }

    // update basis in metadata for update
    #[tokio::test(threaded_scheduler)]
    async fn test_update_basis() {
        // For RegisterReplacedBy with update_basis Header
        // metadata has EntryUpdate on HeaderHash but not EntryHash
        // For RegisterReplacedBy with update_basis Entry
        // metadata has EntryUpdate on EntryHash but not HeaderHash
    }
}
