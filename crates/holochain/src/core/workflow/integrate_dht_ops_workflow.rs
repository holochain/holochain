//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        cascade::Cascade,
        chain_cas::ChainCasBuf,
        dht_op_integration::{
            IntegratedDhtOpsStore, IntegrationQueueStore, IntegrationQueueValue, IntegrationValue,
        },
        metadata::{MetadataBuf, MetadataBufT},
        workspace::{Workspace, WorkspaceResult},
    },
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::{AgentPubKey, Hashable, Hashed};
use holochain_state::{
    buffer::BufferedStore,
    buffer::KvBuf,
    db::{INTEGRATED_DHT_OPS, INTEGRATION_QUEUE},
    prelude::{GetDb, Reader, Writer},
};
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed},
    element::SignedHeaderHashed,
    header::IntendedFor,
    EntryHashed, Header, HeaderHashed, Timestamp,
};
use produce_dht_ops_workflow::dht_op::dht_op_to_light_basis;
use std::convert::TryInto;
use tracing::*;

pub async fn integrate_dht_ops_workflow(
    mut workspace: IntegrateDhtOpsWorkspace<'_>,
    writer: OneshotWriter,
    trigger_publish: &mut TriggerSender,
    agent_pub_key: AgentPubKey,
) -> WorkflowResult<WorkComplete> {
    let result = integrate_dht_ops_workflow_inner(&mut workspace, agent_pub_key).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    // TODO: only trigger if we have integrated ops that we have authored
    trigger_publish.trigger();

    Ok(result)
}

#[instrument(skip(workspace))]
async fn integrate_dht_ops_workflow_inner(
    workspace: &mut IntegrateDhtOpsWorkspace<'_>,
    agent_pub_key: AgentPubKey,
) -> WorkflowResult<WorkComplete> {
    debug!("Starting integrate dht ops workflow");
    // Pull ops out of queue
    // TODO: PERF: Not collect, iterator cannot cross awaits
    // Find a way to do this.
    let ops = workspace
        .integration_queue
        .drain_iter_reverse()?
        .collect::<Vec<_>>()?;

    for value in ops {
        // Process each op
        let IntegrationQueueValue {
            op,
            validation_status,
        } = value;

        let (op, op_hash) = DhtOpHashed::with_data(op).await.into_inner();
        debug!(?op_hash);
        debug!(?op);

        // TODO: PERF: We don't really need this clone because dht_to_op_light_basis could
        // return the full op as it's not consumed when making hashes

        match op.clone() {
            DhtOp::StoreElement(signature, header, maybe_entry) => {
                let header = HeaderHashed::with_data(header).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                let entry_hashed = match maybe_entry {
                    Some(entry) => Some(EntryHashed::with_data(*entry).await?),
                    None => None,
                };
                // Store the entry
                workspace.cas.put(signed_header, entry_hashed)?;
            }
            DhtOp::StoreEntry(signature, new_entry_header, entry) => {
                // Reference to headers
                workspace
                    .meta
                    .register_header(new_entry_header.clone())
                    .await?;

                let header = HeaderHashed::with_data(new_entry_header.into()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                let entry = EntryHashed::with_data(*entry).await?;
                // Store Header and Entry
                workspace.cas.put(signed_header, Some(entry))?;
            }
            DhtOp::RegisterAgentActivity(signature, header) => {
                // Store header
                let header_hashed = HeaderHashed::with_data(header.clone()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header_hashed, signature);
                workspace.cas.put(signed_header, None)?;

                // register agent activity on this agents pub key
                workspace
                    .meta
                    .register_activity(header, agent_pub_key.clone())
                    .await?;
            }
            DhtOp::RegisterReplacedBy(_, entry_update, _) => {
                let old_entry_hash = match entry_update.intended_for {
                    IntendedFor::Header => None,
                    IntendedFor::Entry => {
                        match workspace
                            .cas
                            .get_header(&entry_update.replaces_address)
                            .await?
                            // Handle missing old entry header. Same reason as below
                            .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()))
                        {
                            Some(e) => Some(e),
                            // Handle missing old Entry (Probably StoreEntry hasn't arrived been processed)
                            // This is put the op back in the integration queue to try again later
                            None => {
                                workspace.integration_queue.put(
                                    (Timestamp::now(), op_hash).try_into()?,
                                    IntegrationQueueValue {
                                        validation_status,
                                        op,
                                    },
                                )?;
                                continue;
                            }
                        }
                    }
                };
                workspace
                    .meta
                    .add_update(entry_update, old_entry_hash)
                    .await?;
            }
            DhtOp::RegisterDeletedBy(_, entry_delete) => {
                let entry_hash = match workspace
                    .cas
                    .get_header(&entry_delete.removes_address)
                    .await?
                    // Handle missing entry header. Same reason as below
                    .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()))
                {
                    Some(e) => e,
                    // TODO: VALIDATION: This could also be an invalid delete on a header without a delete
                    // Handle missing Entry (Probably StoreEntry hasn't arrived been processed)
                    // This is put the op back in the integration queue to try again later
                    None => {
                        workspace.integration_queue.put(
                            (Timestamp::now(), op_hash).try_into()?,
                            IntegrationQueueValue {
                                validation_status,
                                op,
                            },
                        )?;
                        continue;
                    }
                };
                workspace.meta.add_delete(entry_delete, entry_hash).await?
            }
            DhtOp::RegisterDeletedHeaderBy(_, entry_delete) => {
                workspace.meta.add_header_delete(entry_delete).await?
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                workspace.meta.add_link(link_add.clone()).await?;
                // Store add Header
                let header = HeaderHashed::with_data(link_add.into()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                workspace.cas.put(signed_header, None)?;
            }
            DhtOp::RegisterRemoveLink(signature, link_remove) => {
                // Check whether they have the base address in the cas.
                // If not then this should put the op back on the queue with a
                // warning that it's unimplemented and later add this to the cache meta.
                // TODO: Base might be in cas due to this agent being an authority for a
                // header on the Base
                if let None = workspace.cas.get_entry(&link_remove.base_address).await? {
                    warn!(
                        "Storing link data when not an author or authority requires the
                         cache metadata store.
                         The cache metadata store is currently unimplemented"
                    );
                    // Add op back on queue
                    workspace.integration_queue.put(
                        (Timestamp::now(), op_hash).try_into()?,
                        IntegrationQueueValue {
                            validation_status,
                            op,
                        },
                    )?;
                    continue;
                }

                // Store link delete Header
                let header = HeaderHashed::with_data(link_remove.clone().into()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                workspace.cas.put(signed_header, None)?;
                let link_add = match workspace
                    .cas
                    .get_header(&link_remove.link_add_address)
                    .await?
                {
                    Some(link_add) => link_add.into_header_and_signature().0.into_content(),
                    // Handle link add missing
                    // Probably just waiting on StoreElement to arrive so put
                    // back in queue with a log message
                    None => {
                        // Add op back on queue
                        workspace.integration_queue.put(
                            (Timestamp::now(), op_hash).try_into()?,
                            IntegrationQueueValue {
                                validation_status,
                                op,
                            },
                        )?;
                        continue;
                    }
                };

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
        }

        // TODO: Instead of using the cascade use the cas and don't error
        // The op should just be put back on the queue if the old entry isn't found
        let (op, basis) = dht_op_to_light_basis(op, &workspace.cascade()).await?;
        let value = IntegrationValue {
            validation_status,
            basis,
            op,
        };
        debug!(msg = "writing", ?op_hash);
        workspace.integrated_dht_ops.put(op_hash, value)?;
    }

    debug!("complete");
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
    fn cascade(&self) -> Cascade {
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
        // flush cas
        self.cas.flush_to_txn(writer)?;
        // flush metadata store
        self.meta.flush_to_txn(writer)?;
        // flush integrated
        self.integrated_dht_ops.flush_to_txn(writer)?;
        // flush integration queue
        self.integration_queue.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        conductor::{
            api::{
                AdminInterfaceApi, AdminRequest, AdminResponse, AppInterfaceApi, AppRequest,
                RealAdminInterfaceApi, RealAppInterfaceApi,
            },
            ConductorBuilder,
        },
        core::{
            ribosome::{NamedInvocation, ZomeCallInvocationFixturator},
            state::{metadata::LinkMetaKey, source_chain::SourceChain},
            SourceChainError,
        },
        fixt::{EntryCreateFixturator, EntryFixturator},
    };
    use fixt::prelude::*;
    use holo_hash::{AgentPubKeyFixturator, Hashable, Hashed};
    use holochain_state::{
        buffer::BufferedStore,
        env::{ReadManager, WriteManager},
        error::DatabaseError,
        test_utils::{test_cell_env, test_conductor_env, test_wasm_env, TestEnvironment},
    };
    use holochain_types::{
        app::{InstallAppDnaPayload, InstallAppPayload},
        dht_op::{DhtOp, DhtOpHashed},
        fixt::{AppEntryTypeFixturator, SignatureFixturator},
        header::{builder, EntryType, NewEntryHeader},
        observability,
        test_utils::{fake_agent_pubkey_1, fake_dna_zomes, write_fake_dna_file},
        validate::ValidationStatus,
        Entry, EntryHashed, Timestamp,
    };
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::HostInput;
    use matches::assert_matches;
    use std::convert::TryInto;
    use unwrap_to::unwrap_to;
    use uuid::Uuid;

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
        let val = IntegrationQueueValue {
            validation_status: ValidationStatus::Valid,
            op: store_entry.clone(),
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

        // Call workflow
        {
            let reader = env_ref.reader().unwrap();
            let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
            let (mut qt, _rx) = TriggerSender::new();
            integrate_dht_ops_workflow(workspace, env.clone().into(), &mut qt, fixt!(AgentPubKey))
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
    #[ignore]
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
        todo!()
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    async fn test_integrate_single_register_replaced_by_for_header() {
        // For RegisterReplacedBy with intended_for Header
        // metadata has EntryUpdate on HeaderHash but not EntryHash
        todo!()
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    async fn test_integrate_single_register_replaced_by_for_entry() {
        // For RegisterReplacedBy with intended_for Entry
        // metadata has EntryUpdate on EntryHash but not HeaderHash
        todo!()
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    async fn test_integrate_single_register_deleted_by() {
        // For RegisterDeletedBy
        // metadata has EntryDelete on HeaderHash
        todo!()
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    async fn test_integrate_single_register_add_link() {
        // For RegisterAddLink
        // metadata has link on EntryHash
        todo!()
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    async fn test_integrate_single_register_remove_link() {
        // For RegisterAddLink
        // metadata has link on EntryHash
        todo!()
    }

    // Integration
    #[tokio::test(threaded_scheduler)]
    async fn commit_entry_add_link() {
        observability::test_run().ok();
        let test_env = test_conductor_env();
        let _tmpdir = test_env.tmpdir.clone();
        let TestEnvironment {
            env: wasm_env,
            tmpdir: _tmpdir,
        } = test_wasm_env();
        let conductor = ConductorBuilder::new()
            .test(test_env, wasm_env)
            .await
            .unwrap();
        let interface = RealAdminInterfaceApi::new(conductor.clone());
        let app_interface = RealAppInterfaceApi::new(conductor.clone());

        // Create dna
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );

        // Install Dna
        let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let dna_payload = InstallAppDnaPayload::path_only(fake_dna_path, "".to_string());
        let agent_key = fake_agent_pubkey_1();
        let payload = InstallAppPayload {
            dnas: vec![dna_payload],
            app_id: "test".to_string(),
            agent_key: agent_key.clone(),
        };
        let request = AdminRequest::InstallApp(Box::new(payload));
        let r = interface.handle_admin_request(request).await;
        debug!(?r);
        let installed_app = unwrap_to!(r => AdminResponse::AppInstalled).clone();

        let cell_id = installed_app.cell_data[0].as_id().clone();
        // Activate app
        let request = AdminRequest::ActivateApp {
            app_id: installed_app.app_id,
        };
        let r = interface.handle_admin_request(request).await;
        assert_matches!(r, AdminResponse::AppActivated);

        let mut entry_fixt = SerializedBytesFixturator::new(Predictable).map(|b| Entry::App(b));

        let base_entry = entry_fixt.next().unwrap();
        let base_entry_hash = EntryHashed::with_data(base_entry.clone())
            .await
            .unwrap()
            .into_hash();
        let target_entry = entry_fixt.next().unwrap();
        let target_entry_hash = EntryHashed::with_data(target_entry.clone())
            .await
            .unwrap()
            .into_hash();
        // Put commit entry into source chain
        {
            let cell_env = conductor.get_cell_env(&cell_id).await.unwrap();
            let dbs = cell_env.dbs().await;
            let env_ref = cell_env.guard().await;

            let reader = env_ref.reader().unwrap();
            let mut sc = SourceChain::new(&reader, &dbs).unwrap();

            let header_builder = builder::EntryCreate {
                entry_type: EntryType::App(fixt!(AppEntryType)),
                entry_hash: base_entry_hash.clone(),
            };
            sc.put(header_builder, Some(base_entry.clone()))
                .await
                .unwrap();

            let header_builder = builder::EntryCreate {
                entry_type: EntryType::App(fixt!(AppEntryType)),
                entry_hash: target_entry_hash.clone(),
            };
            sc.put(header_builder, Some(target_entry.clone()))
                .await
                .unwrap();

            let header_builder = builder::LinkAdd {
                base_address: base_entry_hash.clone(),
                target_address: target_entry_hash.clone(),
                zome_id: 0.into(),
                tag: BytesFixturator::new(Unpredictable).next().unwrap().into(),
            };
            sc.put(header_builder, None).await.unwrap();
            env_ref
                .with_commit::<SourceChainError, _, _>(|writer| {
                    sc.flush_to_txn(writer)?;
                    Ok(())
                })
                .unwrap();
        }

        // Call zome to trigger a the produce workflow
        let request = Box::new(
            ZomeCallInvocationFixturator::new(NamedInvocation(
                cell_id.clone(),
                TestWasm::Foo,
                "foo".into(),
                HostInput::new(fixt!(SerializedBytes)),
            ))
            .next()
            .unwrap(),
        );
        let request = AppRequest::ZomeCallInvocation(request);
        let r = app_interface.handle_app_request(request).await;
        debug!(?r);

        tokio::time::delay_for(std::time::Duration::from_secs(4)).await;

        // Check the ops
        {
            let cell_env = conductor.get_cell_env(&cell_id).await.unwrap();
            let dbs = cell_env.dbs().await;
            let env_ref = cell_env.guard().await;

            let reader = env_ref.reader().unwrap();
            let db = dbs.get_db(&*INTEGRATED_DHT_OPS).unwrap();
            let ops_db = IntegratedDhtOpsStore::new(&reader, db).unwrap();
            let ops = ops_db.iter().unwrap().collect::<Vec<_>>().unwrap();
            debug!(?ops);
            assert!(!ops.is_empty());

            let meta = MetadataBuf::primary(&reader, &dbs).unwrap();
            let key = LinkMetaKey::Base(&base_entry_hash);
            let links = meta.get_links(&key).unwrap();
            let link = links[0].clone();
            assert_eq!(link.target, target_entry_hash);

            let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
            let cascade = workspace.cascade();
            let links = cascade.dht_get_links(&key).await.unwrap();
            let link = links[0].clone();
            assert_eq!(link.target, target_entry_hash);

            let e = cascade.dht_get(&target_entry_hash).await.unwrap().unwrap();
            assert_eq!(e.into_content(), target_entry);

            let e = cascade.dht_get(&base_entry_hash).await.unwrap().unwrap();
            assert_eq!(e.into_content(), base_entry);
        }
    }
}
