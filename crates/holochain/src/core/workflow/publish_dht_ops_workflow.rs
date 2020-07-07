//! # Publish Dht Op Workflow
//!
//! ## Open questions
//! - [x] Publish add and remove links on private entries, what are the constraints on when to publish
//! For now, Publish links on private entries
// TODO: B-01827 Make story about: later consider adding a flag to make a link private and not publish it.
//       Even for those private links, we may need to publish them to the author of the private entry
//       (and we'd have to reference its header  which actually exists on the DHT to make that work,
//       rather than the entry which does not exist on the DHT).
//!
//!

use super::{
    error::WorkflowResult,
    produce_dht_ops_workflow::dht_op_light::{error::DhtOpConvertError, light_to_op},
};
use crate::core::{
    queue_consumer::WorkComplete,
    state::{
        chain_cas::ChainCasBuf,
        dht_op_integration::{AuthoredDhtOpsStore, IntegratedDhtOpsStore, IntegratedDhtOpsValue},
    },
};
use fallible_iterator::FallibleIterator;
use holo_hash::{DhtOpHash, HoloHashBaseExt};
use holochain_p2p::HolochainP2pCell;
use holochain_state::{
    buffer::KvBuf,
    db::{AUTHORED_DHT_OPS, INTEGRATED_DHT_OPS},
    error::DatabaseResult,
    prelude::{GetDb, Reader},
};
use holochain_types::{composite_hash::AnyDhtHash, dht_op::DhtOp};
use std::collections::HashMap;
use tracing::*;

/// Default redundancy factor for validation receipts
// TODO: Pull this from the wasm entry def and only use this if it's missing
// TODO: Put a default in the DnaBundle
// TODO: build zome_types/entry_def map to get the (AppEntryType map to entry def)
pub const DEFAULT_RECEIPT_BUNDLE_SIZE: u32 = 5;

/// Database buffers required for publishing [DhtOp]s
pub struct PublishDhtOpsWorkspace<'env> {
    /// Database of authored [DhtOpHash]
    authored_dht_ops: AuthoredDhtOpsStore<'env>,
    /// Cas for looking up data to construct ops
    cas: ChainCasBuf<'env>,
    // Integrated Ops database for looking up [DhtOp]s
    integrated_dht_ops: IntegratedDhtOpsStore<'env>,
}

pub async fn publish_dht_ops_workflow(
    workspace: PublishDhtOpsWorkspace<'_>,
    network: &mut HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    let to_publish = publish_dht_ops_workflow_inner(&workspace).await?;

    // Commit to the network
    for (basis, ops) in to_publish {
        network.publish(true, basis, ops, None).await?;
    }
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // This workflow doesn't commit anything.
    // Instead it publishes to the network.
    // trigger other workflows
    // (n/a)

    Ok(WorkComplete::Complete)
}

/// Read the authored for ops with receipt count < R
pub async fn publish_dht_ops_workflow_inner(
    workspace: &PublishDhtOpsWorkspace<'_>,
) -> WorkflowResult<HashMap<AnyDhtHash, Vec<(DhtOpHash, DhtOp)>>> {
    // TODO: PERF: We need to check all ops every time this runs
    // instead we could have a queue of ops where count < R and a kv for count > R.
    // Then if the count for an ops reduces below R move it to the queue.
    let ops = workspace
        .authored()
        .iter()?
        .filter_map(|(k, r)| {
            Ok(if r < DEFAULT_RECEIPT_BUNDLE_SIZE {
                Some(k)
            } else {
                None
            })
        })
        .collect::<Vec<_>>()?;

    // Ops to publish by basis
    let mut to_publish = HashMap::new();

    for op in ops {
        // Deserialize DhtOpHash
        let op_hash = DhtOpHash::with_pre_hashed(op.to_vec());

        // Reconstruct the DhtOp
        let op = match workspace.integrated().get(&op_hash)? {
            Some(op) => op,
            None => {
                trace!(
                    "DhtOpHash {:?} in authored but not yet in integrated",
                    op_hash
                );
                continue;
            }
        };
        let IntegratedDhtOpsValue { basis, op, .. } = op;
        let op = match light_to_op(op, workspace.cas()).await {
            // Ignore StoreEntry ops on private
            Err(DhtOpConvertError::StoreEntryOnPrivate) => continue,
            r => r?,
        };

        // For every op publish a request
        // Collect and sort ops by basis
        to_publish
            .entry(basis)
            .or_insert_with(Vec::new)
            .push((op_hash, op));
    }

    Ok(to_publish)
}

impl<'env> PublishDhtOpsWorkspace<'env> {
    // Create a constructor that only has gives access to public entries
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let db = dbs.get_db(&*AUTHORED_DHT_OPS)?;
        let authored_dht_ops = KvBuf::new(reader, db)?;
        // Note that this must always be false as we don't want private entries being published
        let cas = ChainCasBuf::primary(reader, dbs, false)?;
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBuf::new(reader, db)?;
        Ok(Self {
            authored_dht_ops,
            cas,
            integrated_dht_ops,
        })
    }

    fn authored(&self) -> &AuthoredDhtOpsStore<'env> {
        &self.authored_dht_ops
    }

    fn integrated(&self) -> &IntegratedDhtOpsStore<'env> {
        &self.integrated_dht_ops
    }

    fn cas(&self) -> &ChainCasBuf<'env> {
        &self.cas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::workflow::produce_dht_ops_workflow::dht_op_light::{
            dht_op_to_light_basis, DhtOpLight,
        },
        fixt::{EntryCreateFixturator, EntryFixturator, EntryUpdateFixturator, LinkAddFixturator},
    };
    use fixt::prelude::*;
    use futures::future::FutureExt;
    use ghost_actor::GhostControlSender;
    use holo_hash::{AgentPubKeyFixturator, DnaHashFixturator, Hashable, Hashed};
    use holochain_p2p::{
        actor::{HolochainP2p, HolochainP2pRefToCell, HolochainP2pSender},
        spawn_holochain_p2p,
    };
    use holochain_state::{
        buffer::BufferedStore,
        env::{EnvironmentWriteRef, ReadManager, WriteManager},
        error::DatabaseError,
        test_utils::test_cell_env,
    };
    use holochain_types::{
        dht_op::{DhtOp, DhtOpHashed},
        element::SignedHeaderHashed,
        fixt::{AppEntryTypeFixturator, SignatureFixturator},
        header::{EntryType, IntendedFor, NewEntryHeader},
        observability,
        validate::ValidationStatus,
        EntryHashed, Header, HeaderHashed,
    };
    use holochain_zome_types::entry_def::EntryVisibility;
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicU32, Ordering},
            Arc,
        },
        time::Duration,
    };
    use test_case::test_case;
    use tokio::task::JoinHandle;

    const RECV_TIMEOUT: Duration = Duration::from_millis(3000);

    /// publish ops setup
    async fn setup<'env>(
        env_ref: &EnvironmentWriteRef<'env>,
        dbs: &impl GetDb,
        num_agents: u32,
        num_hash: u32,
        panic_on_publish: bool,
    ) -> (
        ghost_actor::GhostSender<HolochainP2p>,
        HolochainP2pCell,
        JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        // Create data fixts for op
        let mut sig_fixt = SignatureFixturator::new(Unpredictable);
        let mut link_add_fixt = LinkAddFixturator::new(Unpredictable);

        let mut data = Vec::new();
        for _ in 0..num_hash {
            // Create data for op
            let sig = sig_fixt.next().unwrap();
            let link_add = link_add_fixt.next().unwrap();
            // Create DhtOp
            let op = DhtOp::RegisterAddLink(sig.clone(), link_add.clone());
            // Get the hash from the op
            let op_hashed = DhtOpHashed::with_data(op.clone()).await;
            // Convert op to DhtOpLight
            let header_hash = HeaderHashed::with_data(Header::LinkAdd(link_add.clone()))
                .await
                .unwrap();
            let light = IntegratedDhtOpsValue {
                validation_status: ValidationStatus::Valid,
                basis: link_add.base_address.into(),
                op: DhtOpLight::RegisterAddLink(header_hash.as_hash().clone()),
            };
            data.push((sig, op_hashed, light, header_hash));
        }

        // Create and fill authored ops db in the workspace
        {
            let reader = env_ref.reader().unwrap();
            let mut workspace = PublishDhtOpsWorkspace::new(&reader, dbs).unwrap();
            for (sig, op_hashed, light, header_hash) in data {
                let op_hash = op_hashed.as_hash().clone();
                workspace.authored_dht_ops.put(op_hash.clone(), 0).unwrap();
                // Put DhtOpLight into the integrated db
                workspace.integrated_dht_ops.put(op_hash, light).unwrap();
                // Put data into cas
                let signed_header = SignedHeaderHashed::with_presigned(header_hash, sig);
                workspace.cas.put(signed_header, None).unwrap();
            }
            // Manually commit because this workspace doesn't commit to all dbs
            env_ref
                .with_commit::<DatabaseError, _, _>(|writer| {
                    workspace.authored_dht_ops.flush_to_txn(writer)?;
                    workspace.integrated_dht_ops.flush_to_txn(writer)?;
                    workspace.cas.flush_to_txn(writer)?;
                    Ok(())
                })
                .unwrap();
        }

        // Create cell data
        let dna = fixt!(DnaHash);
        let agents = AgentPubKeyFixturator::new(Unpredictable)
            .take(num_agents as usize)
            .collect::<Vec<_>>();

        // Create the network
        let (network, mut recv) = spawn_holochain_p2p().await.unwrap();
        let (tx_complete, rx_complete) = tokio::sync::oneshot::channel();
        let cell_network = network.to_cell(dna.clone(), agents[0].clone());
        let mut recv_count: u32 = 0;
        let total_expected = num_agents * num_hash;

        // Receive events and increment count
        let recv_task = tokio::task::spawn({
            async move {
                use tokio::stream::StreamExt;
                let mut tx_complete = Some(tx_complete);
                while let Some(evt) = recv.next().await {
                    use holochain_p2p::event::HolochainP2pEvent::*;
                    match evt {
                        Publish { respond, .. } => {
                            respond.respond(Ok(async move { Ok(()) }.boxed().into()));
                            if panic_on_publish {
                                panic!("Published, when expecting not to")
                            }
                            recv_count += 1;
                            if recv_count == total_expected {
                                // notify the test that all items have been received
                                tx_complete.take().unwrap().send(()).unwrap()
                            }
                        }
                        _ => panic!("unexpected event"),
                    }
                }
            }
        });

        // Join some agents onto the network
        for agent in agents {
            network.join(dna.clone(), agent).await.unwrap();
        }

        (network, cell_network, recv_task, rx_complete)
    }

    /// Call the workflow
    async fn call_workflow<'env>(
        env_ref: &EnvironmentWriteRef<'env>,
        dbs: &impl GetDb,
        mut cell_network: HolochainP2pCell,
    ) {
        let reader = env_ref.reader().unwrap();
        let mut workspace = PublishDhtOpsWorkspace::new(&reader, dbs).unwrap();
        let to_publish = publish_dht_ops_workflow_inner(&mut workspace)
            .await
            .unwrap();

        for (basis, ops) in to_publish {
            cell_network.publish(true, basis, ops, None).await.unwrap();
        }
    }

    /// There is a test that shows that network messages would be sent to all agents via broadcast.
    #[test_case(1, 1)]
    #[test_case(1, 10)]
    #[test_case(1, 100)]
    #[test_case(10, 1)]
    #[test_case(10, 10)]
    #[test_case(10, 100)]
    #[test_case(100, 1)]
    #[test_case(100, 10)]
    #[test_case(100, 100)]
    fn test_sent_to_r_nodes(num_agents: u32, num_hash: u32) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            observability::test_run().ok();

            // Create test env
            let env = test_cell_env();
            let dbs = env.dbs().await;
            let env_ref = env.guard().await;

            // Setup
            let (network, cell_network, recv_task, rx_complete) =
                setup(&env_ref, &dbs, num_agents, num_hash, false).await;

            call_workflow(&env_ref, &dbs, cell_network).await;

            // Wait for expected # of responses, or timeout
            tokio::select! {
                _ = rx_complete => {}
                _ = tokio::time::delay_for(RECV_TIMEOUT) => {
                    panic!("Timed out while waiting for expected responses.")
                }
            };

            // Shutdown
            network.ghost_actor_shutdown().await.unwrap();
            recv_task.await.unwrap();
        });
    }

    /// There is a test that shows that if the validation_receipt_count > R
    /// for a DHTOp we don't re-publish it
    #[test_case(1, 1)]
    #[test_case(1, 10)]
    #[test_case(1, 100)]
    #[test_case(10, 1)]
    #[test_case(10, 10)]
    #[test_case(10, 100)]
    #[test_case(100, 1)]
    #[test_case(100, 10)]
    #[test_case(100, 100)]
    fn test_no_republish(num_agents: u32, num_hash: u32) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            observability::test_run().ok();

            // Create test env
            let env = test_cell_env();
            let dbs = env.dbs().await;
            let env_ref = env.guard().await;

            // Setup
            let (network, cell_network, recv_task, _) =
                setup(&env_ref, &dbs, num_agents, num_hash, true).await;

            // Update the authored to have > R counts
            {
                let reader = env_ref.reader().unwrap();
                let mut workspace = PublishDhtOpsWorkspace::new(&reader, &dbs).unwrap();

                // Update authored to R
                let ops = workspace
                    .authored_dht_ops
                    .iter()
                    .unwrap()
                    .map(|(k, _)| Ok(DhtOpHash::with_pre_hashed(k.to_vec())))
                    .collect::<Vec<_>>()
                    .unwrap();

                for op in ops {
                    workspace
                        .authored_dht_ops
                        .put(op, DEFAULT_RECEIPT_BUNDLE_SIZE)
                        .unwrap();
                }

                // Manually commit because this workspace doesn't commit to all dbs
                env_ref
                    .with_commit::<DatabaseError, _, _>(|writer| {
                        workspace.authored_dht_ops.flush_to_txn(writer)?;
                        Ok(())
                    })
                    .unwrap();
            }

            // Call the workflow
            call_workflow(&env_ref, &dbs, cell_network).await;

            // If we can wait a while without receiving any publish, we have succeeded
            tokio::time::delay_for(Duration::from_millis(
                std::cmp::min(50, std::cmp::max(2000, 10 * num_agents * num_hash)).into(),
            ))
            .await;

            // Shutdown
            network.ghost_actor_shutdown().await.unwrap();
            recv_task.await.unwrap();
        });
    }

    /// There is a test to shows that DHTOps that were produced on private entries are not published.
    /// Some do get published
    /// Current private constraints:
    /// - No private Entry is ever published in any op
    /// - No StoreEntry
    /// - This workflow does not have access to private entries
    /// - Add / Remove links: Currently publish all.
    /// ## Explication
    /// This test is a little big so a quick run down:
    /// 1. All ops that can contain entries are created with entries (StoreElement, StoreEntry and RegisterReplacedBy)
    /// 2. Then we create identical versions of these ops without the entires (set to None) (expect StoreEntry)
    /// 3. The workflow is run and the ops are sent to the network receiver
    /// 4. We check that the correct number of ops are received (so we know there were no other ops sent)
    /// 5. StoreEntry is __not__ expected so would show up as an extra if it was produced
    /// 6. Every op that is received (StoreElement and RegisterReplacedBy) is checked to match the expected versions (entries removed)
    /// 7. Each op also has a count to check for duplicates
    #[test_case(1)]
    #[test_case(10)]
    #[test_case(100)]
    fn test_private_entries(num_agents: u32) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            observability::test_run().ok();

            // Create test env
            let env = test_cell_env();
            let dbs = env.dbs().await;
            let env_ref = env.guard().await;

            // Setup data
            let mut sig_fixt = SignatureFixturator::new(Unpredictable);
            let sig = sig_fixt.next().unwrap();
            let original_entry = fixt!(Entry);
            let new_entry = fixt!(Entry);
            let original_entry_hashed = EntryHashed::with_data(original_entry.clone())
                .await
                .unwrap();
            let new_entry_hashed = EntryHashed::with_data(new_entry.clone()).await.unwrap();

            // Create StoreElement
            // Create the headers
            let mut entry_create = fixt!(EntryCreate);
            let mut entry_update = fixt!(EntryUpdate);

            // Make them private
            let visibility = EntryVisibility::Private;
            let mut entry_type_fixt =
                AppEntryTypeFixturator::new(visibility.clone()).map(EntryType::App);
            entry_create.entry_type = entry_type_fixt.next().unwrap();
            entry_update.entry_type = entry_type_fixt.next().unwrap();

            // Point update at entry
            entry_update.intended_for = IntendedFor::Header;

            // Update the entry hashes
            entry_create.entry_hash = original_entry_hashed.as_hash().clone();
            entry_update.entry_hash = new_entry_hashed.as_hash().clone();

            let entry_create_header = Header::EntryCreate(entry_create.clone());

            // Put data in cas
            {
                let reader = env_ref.reader().unwrap();

                let mut cas = ChainCasBuf::primary(&reader, &dbs, true).unwrap();

                let header_hash = HeaderHashed::with_data(entry_create_header.clone())
                    .await
                    .unwrap();

                // Update the replaces to the header of the original
                entry_update.replaces_address = header_hash.as_hash().clone();

                // Put data into cas
                let signed_header = SignedHeaderHashed::with_presigned(header_hash, sig.clone());
                cas.put(signed_header, Some(original_entry_hashed)).unwrap();

                let entry_update_header = Header::EntryUpdate(entry_update.clone());
                let header_hash = HeaderHashed::with_data(entry_update_header).await.unwrap();
                let signed_header = SignedHeaderHashed::with_presigned(header_hash, sig.clone());
                cas.put(signed_header, Some(new_entry_hashed)).unwrap();
                env_ref
                    .with_commit::<DatabaseError, _, _>(|writer| {
                        cas.flush_to_txn(writer)?;
                        Ok(())
                    })
                    .unwrap();
            }
            let (store_element, store_entry, register_replaced_by) = {
                let reader = env_ref.reader().unwrap();
                // Create easy way to create test cascade
                let cas = ChainCasBuf::primary(&reader, &dbs, true).unwrap();

                let op = DhtOp::StoreElement(
                    sig.clone(),
                    entry_create_header.clone(),
                    Some(original_entry.clone().into()),
                );
                // Op is expected to not contain the Entry even though the above contains the entry
                let expected_op = DhtOp::StoreElement(sig.clone(), entry_create_header, None);
                let (light, basis) = dht_op_to_light_basis(op.clone(), &cas).await.unwrap();
                let op_hash = DhtOpHashed::with_data(op.clone()).await.into_hash();
                let store_element = (op_hash, light, basis, expected_op);

                // Create StoreEntry
                let header = NewEntryHeader::Create(entry_create.clone());
                let op = DhtOp::StoreEntry(sig.clone(), header, original_entry.clone().into());
                let (light, basis) = dht_op_to_light_basis(op.clone(), &cas).await.unwrap();
                let op_hash = DhtOpHashed::with_data(op.clone()).await.into_hash();
                let store_entry = (op_hash, light, basis);

                // Create RegisterReplacedBy
                let op = DhtOp::RegisterReplacedBy(
                    sig.clone(),
                    entry_update.clone(),
                    Some(new_entry.clone().into()),
                );
                // Op is expected to not contain the Entry even though the above contains the entry
                let expected_op =
                    DhtOp::RegisterReplacedBy(sig.clone(), entry_update.clone(), None);
                let (light, basis) = dht_op_to_light_basis(op.clone(), &cas).await.unwrap();
                let op_hash = DhtOpHashed::with_data(op.clone()).await.into_hash();
                let register_replaced_by = (op_hash, light, basis, expected_op);

                (store_element, store_entry, register_replaced_by)
            };

            // Gather the expected op hashes, ops and basis
            // We are only expecting Store Element and Register Replaced By ops and nothing else
            let store_element_count = Arc::new(AtomicU32::new(0));
            let register_replaced_by_count = Arc::new(AtomicU32::new(0));
            let expected = {
                let mut map = HashMap::new();
                let op_hash = store_element.0.clone();
                let expected_op = store_element.3.clone();
                let basis = store_element.2.clone();
                let store_element_count = store_element_count.clone();
                map.insert(op_hash, (expected_op, basis, store_element_count));
                let op_hash = register_replaced_by.0.clone();
                let expected_op = register_replaced_by.3.clone();
                let basis = register_replaced_by.2.clone();
                let register_replaced_by_count = register_replaced_by_count.clone();
                map.insert(op_hash, (expected_op, basis, register_replaced_by_count));
                map
            };

            // Create and fill authored ops db in the workspace
            {
                let reader = env_ref.reader().unwrap();
                let mut workspace = PublishDhtOpsWorkspace::new(&reader, &dbs).unwrap();
                let (op_hash, light, basis, _) = store_element;
                let integration = IntegratedDhtOpsValue {
                    validation_status: ValidationStatus::Valid,
                    op: light,
                    basis,
                };
                workspace.authored_dht_ops.put(op_hash.clone(), 0).unwrap();
                // Put DhtOpLight into the integrated db
                workspace
                    .integrated_dht_ops
                    .put(op_hash, integration)
                    .unwrap();

                let (op_hash, light, basis) = store_entry;
                let integration = IntegratedDhtOpsValue {
                    validation_status: ValidationStatus::Valid,
                    op: light,
                    basis,
                };
                workspace.authored_dht_ops.put(op_hash.clone(), 0).unwrap();
                // Put DhtOpLight into the integrated db
                workspace
                    .integrated_dht_ops
                    .put(op_hash, integration)
                    .unwrap();

                let (op_hash, light, basis, _) = register_replaced_by;
                let integration = IntegratedDhtOpsValue {
                    validation_status: ValidationStatus::Valid,
                    op: light,
                    basis,
                };
                workspace.authored_dht_ops.put(op_hash.clone(), 0).unwrap();
                // Put DhtOpLight into the integrated db
                workspace
                    .integrated_dht_ops
                    .put(op_hash, integration)
                    .unwrap();
                // Manually commit because this workspace doesn't commit to all dbs
                env_ref
                    .with_commit::<DatabaseError, _, _>(|writer| {
                        workspace.authored_dht_ops.flush_to_txn(writer)?;
                        workspace.integrated_dht_ops.flush_to_txn(writer)?;
                        Ok(())
                    })
                    .unwrap();
            }

            // Create cell data
            let dna = fixt!(DnaHash);
            let agents = AgentPubKeyFixturator::new(Unpredictable)
                .take(num_agents as usize)
                .collect::<Vec<_>>();

            // Create the network
            let (network, mut recv) = spawn_holochain_p2p().await.unwrap();
            let cell_network = network.to_cell(dna.clone(), agents[0].clone());
            let (tx_complete, rx_complete) = tokio::sync::oneshot::channel();
            let total_expected = num_agents;
            let mut recv_count: u32 = 0;

            // Receive events and increment count
            let recv_task = tokio::task::spawn({
                async move {
                    use tokio::stream::StreamExt;
                    let mut tx_complete = Some(tx_complete);
                    while let Some(evt) = recv.next().await {
                        use holochain_p2p::event::HolochainP2pEvent::*;
                        match evt {
                            Publish {
                                respond,
                                span,
                                dht_hash,
                                ops,
                                ..
                            } => {
                                let _g = span.enter();
                                debug!(?dht_hash);
                                debug!(?ops);

                                // Check the ops are correct
                                for (op_hash, op) in ops {
                                    match expected.get(&op_hash) {
                                        Some((expected_op, expected_basis, count)) => {
                                            assert_eq!(&op, expected_op);
                                            assert_eq!(&dht_hash, expected_basis);
                                            count.fetch_add(1, Ordering::SeqCst);
                                        }
                                        None => {
                                            panic!("This DhtOpHash was not expected: {:?}", op_hash)
                                        }
                                    }
                                }
                                respond.respond(Ok(async move { Ok(()) }.boxed().into()));
                                recv_count += 1;
                                if recv_count == total_expected {
                                    tx_complete.take().unwrap().send(()).unwrap();
                                }
                            }
                            _ => panic!("unexpected event"),
                        }
                    }
                }
            });

            // Join some agents onto the network
            for agent in agents {
                network.join(dna.clone(), agent).await.unwrap();
            }

            call_workflow(&env_ref, &dbs, cell_network).await;

            // Wait for expected # of responses, or timeout
            tokio::select! {
                _ = rx_complete => {}
                _ = tokio::time::delay_for(RECV_TIMEOUT) => {
                    panic!("Timed out while waiting for expected responses.")
                }
            };

            // Check there is no ops left that didn't come through
            assert_eq!(
                num_agents,
                register_replaced_by_count.load(Ordering::SeqCst)
            );
            assert_eq!(num_agents, store_element_count.load(Ordering::SeqCst));

            // Shutdown
            network.ghost_actor_shutdown().await.unwrap();
            recv_task.await.unwrap();
        });
    }

    // TODO: COVERAGE: Test public ops do publish
}
