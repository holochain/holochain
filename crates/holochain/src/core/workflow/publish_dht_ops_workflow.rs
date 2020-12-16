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

use super::error::WorkflowResult;
use super::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertError;
use super::produce_dht_ops_workflow::dht_op_light::light_to_op;
use crate::core::queue_consumer::OneshotWriter;
use crate::core::queue_consumer::WorkComplete;
use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_lmdb::buffer::BufferedStore;
use holochain_lmdb::buffer::KvBufFresh;
use holochain_lmdb::db::AUTHORED_DHT_OPS;
use holochain_lmdb::fresh_reader;
use holochain_lmdb::prelude::*;
use holochain_lmdb::transaction::Writer;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pCellT;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use std::collections::HashMap;
use std::time;
use tracing::*;

/// Default redundancy factor for validation receipts
// TODO: Pull this from the wasm entry def and only use this if it's missing
// TODO: Put a default in the DnaBundle
// TODO: build zome_types/entry_def map to get the (AppEntryType map to entry def)
pub const DEFAULT_RECEIPT_BUNDLE_SIZE: u32 = 5;

/// Don't publish a DhtOp more than once during this interval.
/// This allows us to trigger the publish workflow as often as we like, without
/// flooding the network with spurious publishes.
pub const MIN_PUBLISH_INTERVAL: time::Duration = time::Duration::from_secs(5);

/// Database buffers required for publishing [DhtOp]s
pub struct PublishDhtOpsWorkspace {
    /// Database of authored DhtOps, with data about prior publishing
    authored_dht_ops: AuthoredDhtOpsStore,
    /// Element store for looking up data to construct ops
    elements: ElementBuf<AuthoredPrefix>,
}

#[instrument(skip(workspace, writer, network))]
pub async fn publish_dht_ops_workflow(
    mut workspace: PublishDhtOpsWorkspace,
    writer: OneshotWriter,
    network: &mut HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    let to_publish = publish_dht_ops_workflow_inner(&mut workspace).await?;

    // Commit to the network
    for (basis, ops) in to_publish {
        network.publish(true, basis, ops, None).await?;
    }
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer.with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))?;

    Ok(WorkComplete::Complete)
}

/// Read the authored for ops with receipt count < R
pub async fn publish_dht_ops_workflow_inner(
    workspace: &mut PublishDhtOpsWorkspace,
) -> WorkflowResult<HashMap<AnyDhtHash, Vec<(DhtOpHash, DhtOp)>>> {
    // TODO: PERF: We need to check all ops every time this runs
    // instead we could have a queue of ops where count < R and a kv for count > R.
    // Then if the count for an ops reduces below R move it to the queue.
    let now = timestamp::now();
    // chrono cannot create const durations
    let interval =
        chrono::Duration::from_std(MIN_PUBLISH_INTERVAL).expect("const interval must be positive");

    // one of many ways to access the env
    let env = workspace.elements.headers().env().clone();

    let values = fresh_reader!(env, |r| workspace
        .authored()
        .iter(&r)?
        .filter_map(|(k, mut r)| {
            Ok(if r.receipt_count < DEFAULT_RECEIPT_BUNDLE_SIZE {
                let needs_publish = r
                    .last_publish_time
                    .and_then(|last| now.checked_difference_signed(&last))
                    .and_then(|duration| Some(duration > interval))
                    .unwrap_or(true);
                if needs_publish {
                    r.last_publish_time = Some(now);
                    // HACK: Incrementing the receipt count to prevent publishing
                    // forever although without receipts this could lead to data loss
                    // and relies on gossip for data integrity.
                    // This should be removed when receipts are implemented.
                    r.receipt_count += 1;
                    Some((DhtOpHash::from_raw_39_panicky(k.to_vec()), r))
                } else {
                    None
                }
            } else {
                None
            })
        })
        .collect::<Vec<_>>())?;

    // Ops to publish by basis
    let mut to_publish = HashMap::new();

    for (op_hash, value) in values {
        // Insert updated values into database for items about to be published
        let op = value.op.clone();
        workspace.authored().put(op_hash.clone(), value)?;

        let op = match light_to_op(op, workspace.elements()) {
            // Ignore StoreEntry ops on private
            Err(DhtOpConvertError::StoreEntryOnPrivate) => continue,
            r => r?,
        };
        // For every op publish a request
        // Collect and sort ops by basis
        to_publish
            .entry(op.dht_basis())
            .or_insert_with(Vec::new)
            .push((op_hash, op));
    }

    Ok(to_publish)
}

impl Workspace for PublishDhtOpsWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.authored_dht_ops.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

impl PublishDhtOpsWorkspace {
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let db = env.get_db(&*AUTHORED_DHT_OPS)?;
        let authored_dht_ops = KvBufFresh::new(env.clone(), db);
        // Note that this must always be false as we don't want private entries being published
        let elements = ElementBuf::authored(env, false)?;
        Ok(Self {
            authored_dht_ops,
            elements,
        })
    }

    fn authored(&mut self) -> &mut AuthoredDhtOpsStore {
        &mut self.authored_dht_ops
    }

    fn elements(&self) -> &ElementBuf<AuthoredPrefix> {
        &self.elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::queue_consumer::TriggerSender;
    use crate::core::workflow::fake_genesis;
    use crate::core::workflow::produce_dht_ops_workflow::produce_dht_ops_workflow;
    use crate::core::workflow::produce_dht_ops_workflow::ProduceDhtOpsWorkspace;
    use crate::core::SourceChainError;
    use crate::fixt::CreateLinkFixturator;
    use crate::fixt::EntryFixturator;
    use crate::test_utils::test_network_with_events;
    use crate::test_utils::TestNetwork;
    use ::fixt::prelude::*;
    use futures::future::FutureExt;
    use holochain_p2p::actor::HolochainP2pSender;
    use holochain_p2p::HolochainP2pRef;
    use matches::assert_matches;
    use observability;
    use std::collections::HashMap;
    use std::convert::TryInto;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::Duration;
    use test_case::test_case;
    use tokio::task::JoinHandle;
    use tracing_futures::Instrument;

    const RECV_TIMEOUT: Duration = Duration::from_millis(3000);

    /// publish ops setup
    async fn setup<'env>(
        env: EnvironmentWrite,
        num_agents: u32,
        num_hash: u32,
        panic_on_publish: bool,
    ) -> (
        TestNetwork,
        HolochainP2pCell,
        JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        let env_ref = env.guard();
        // Create data fixts for op
        let mut sig_fixt = SignatureFixturator::new(Unpredictable);
        let mut link_add_fixt = CreateLinkFixturator::new(Unpredictable);

        let mut data = Vec::new();
        for _ in 0..num_hash {
            // Create data for op
            let sig = sig_fixt.next().unwrap();
            let link_add = link_add_fixt.next().unwrap();
            // Create DhtOp
            let op = DhtOp::RegisterAddLink(sig.clone(), link_add.clone());
            // Get the hash from the op
            let op_hashed = DhtOpHashed::from_content_sync(op.clone());
            // Convert op to DhtOpLight
            let header_hash = HeaderHashed::from_content_sync(link_add.clone().into());
            let op_light = DhtOpLight::RegisterAddLink(
                header_hash.as_hash().clone(),
                link_add.base_address.into(),
            );
            data.push((sig, op_hashed, op_light, header_hash));
        }

        // Create and fill authored ops db in the workspace
        {
            let mut workspace = PublishDhtOpsWorkspace::new(env.clone().into()).unwrap();
            for (sig, op_hashed, op_light, header_hash) in data {
                let op_hash = op_hashed.as_hash().clone();
                let authored_value = AuthoredDhtOpsValue::from_light(op_light);
                workspace
                    .authored_dht_ops
                    .put(op_hash.clone(), authored_value)
                    .unwrap();
                // Put data into element store
                let signed_header = SignedHeaderHashed::with_presigned(header_hash, sig);
                workspace.elements.put(signed_header, None).unwrap();
            }
            // Manually commit because this workspace doesn't commit to all dbs
            env_ref
                .with_commit::<DatabaseError, _, _>(|writer| {
                    workspace.authored_dht_ops.flush_to_txn(writer)?;
                    workspace.elements.flush_to_txn(writer)?;
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
        let filter_events = |evt: &_| match evt {
            holochain_p2p::event::HolochainP2pEvent::Publish { .. } => true,
            _ => false,
        };
        let (tx, mut recv) = tokio::sync::mpsc::channel(10);
        let test_network = test_network_with_events(
            Some(dna.clone()),
            Some(agents[0].clone()),
            filter_events,
            tx,
        )
        .await;
        let (tx_complete, rx_complete) = tokio::sync::oneshot::channel();
        let cell_network = test_network.cell_network();
        let network = test_network.network();
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
                                tx_complete.take().unwrap().send(()).unwrap();
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        // Join some agents onto the network
        // Skip the first agent as it has already joined
        for agent in agents.into_iter().skip(1) {
            HolochainP2pRef::join(&network, dna.clone(), agent)
                .await
                .unwrap();
        }

        (test_network, cell_network, recv_task, rx_complete)
    }

    /// Call the workflow
    async fn call_workflow(env: EnvironmentWrite, mut cell_network: HolochainP2pCell) {
        let workspace = PublishDhtOpsWorkspace::new(env.clone().into()).unwrap();
        publish_dht_ops_workflow(workspace, env.clone().into(), &mut cell_network)
            .await
            .unwrap();
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
        crate::conductor::tokio_runtime().block_on(async {
            observability::test_run().ok();

            // Create test env
            let test_env = test_cell_env();
            let env = test_env.env();

            // Setup
            let (_network, cell_network, recv_task, rx_complete) =
                setup(env.clone(), num_agents, num_hash, false).await;

            call_workflow(env.clone().into(), cell_network).await;

            // Wait for expected # of responses, or timeout
            tokio::select! {
                _ = rx_complete => {}
                _ = tokio::time::delay_for(RECV_TIMEOUT) => {
                    panic!("Timed out while waiting for expected responses.")
                }
            };

            let check = async move {
                let env_ref = env.guard();
                recv_task.await.unwrap();
                let reader = env_ref.reader().unwrap();
                let mut workspace = PublishDhtOpsWorkspace::new(env.clone().into()).unwrap();
                for i in workspace.authored().iter(&reader).unwrap().iterator() {
                    // Check that each item now has a publish time
                    assert!(i.expect("can iterate").1.last_publish_time.is_some())
                }
            };

            // Shutdown
            tokio::time::timeout(Duration::from_secs(10), check)
                .await
                .ok();
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
        crate::conductor::tokio_runtime().block_on(async {
            observability::test_run().ok();

            // Create test env
            let test_env = test_cell_env();
            let env = test_env.env();
            let env_ref = env.guard();

            // Setup
            let (_network, cell_network, recv_task, _) =
                setup(env.clone(), num_agents, num_hash, true).await;

            // Update the authored to have > R counts
            {
                let reader = env_ref.reader().unwrap();
                let mut workspace = PublishDhtOpsWorkspace::new(env.clone().into()).unwrap();

                // Update authored to R
                let values = workspace
                    .authored_dht_ops
                    .iter(&reader)
                    .unwrap()
                    .map(|(k, mut v)| {
                        v.receipt_count = DEFAULT_RECEIPT_BUNDLE_SIZE;
                        Ok((DhtOpHash::from_raw_39_panicky(k.to_vec()), v))
                    })
                    .collect::<Vec<_>>()
                    .unwrap();

                for (hash, v) in values.into_iter() {
                    workspace.authored_dht_ops.put(hash, v).unwrap();
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
            call_workflow(env.clone().into(), cell_network).await;

            // If we can wait a while without receiving any publish, we have succeeded
            tokio::time::delay_for(Duration::from_millis(
                std::cmp::min(50, std::cmp::max(2000, 10 * num_agents * num_hash)).into(),
            ))
            .await;

            // Shutdown
            tokio::time::timeout(Duration::from_secs(10), recv_task)
                .await
                .ok();
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
    /// 1. All ops that can contain entries are created with entries (StoreElement, StoreEntry and RegisterUpdatedContent)
    /// 2. Then we create identical versions of these ops without the entires (set to None) (expect StoreEntry)
    /// 3. The workflow is run and the ops are sent to the network receiver
    /// 4. We check that the correct number of ops are received (so we know there were no other ops sent)
    /// 5. StoreEntry is __not__ expected so would show up as an extra if it was produced
    /// 6. Every op that is received (StoreElement and RegisterUpdatedContent) is checked to match the expected versions (entries removed)
    /// 7. Each op also has a count to check for duplicates
    #[test_case(1)]
    #[test_case(10)]
    #[test_case(100)]
    fn test_private_entries(num_agents: u32) {
        crate::conductor::tokio_runtime().block_on(
            async {
                observability::test_run().ok();

                // Create test env
                let test_env = test_cell_env();
                let env = test_env.env();
                let env_ref = env.guard();

                // Setup data
                let original_entry = fixt!(Entry);
                let new_entry = fixt!(Entry);
                let original_entry_hash = EntryHash::with_data_sync(&original_entry);
                let new_entry_hash = EntryHash::with_data_sync(&new_entry);

                // Make them private
                let visibility = EntryVisibility::Private;
                let mut entry_type_fixt =
                    AppEntryTypeFixturator::new(visibility.clone()).map(EntryType::App);
                let ec_entry_type = entry_type_fixt.next().unwrap();
                let eu_entry_type = entry_type_fixt.next().unwrap();

                // Genesis and produce ops to clear these from the chains
                {
                    let mut source_chain = SourceChain::new(env.clone().into()).unwrap();
                    fake_genesis(&mut source_chain).await.unwrap();
                    env_ref
                        .with_commit::<SourceChainError, _, _>(|writer| {
                            source_chain.flush_to_txn(writer)?;
                            Ok(())
                        })
                        .unwrap();
                }
                {
                    let workspace = ProduceDhtOpsWorkspace::new(env.clone().into()).unwrap();
                    let (mut qt, _rx) = TriggerSender::new();
                    let complete = produce_dht_ops_workflow(workspace, env.clone().into(), &mut qt)
                        .await
                        .unwrap();
                    assert_matches!(complete, WorkComplete::Complete);
                }
                {
                    let mut workspace = ProduceDhtOpsWorkspace::new(env.clone().into()).unwrap();
                    env_ref
                        .with_commit::<SourceChainError, _, _>(|writer| {
                            workspace.authored_dht_ops.clear_all(writer)?;
                            Ok(())
                        })
                        .unwrap();
                }

                // Put data in elements
                let (entry_create_header, entry_update_header) = {
                    let mut source_chain = SourceChain::new(env.clone().into()).unwrap();
                    let original_header_address = source_chain
                        .put(
                            builder::Create {
                                entry_type: ec_entry_type,
                                entry_hash: original_entry_hash.clone(),
                            },
                            Some(original_entry),
                        )
                        .await
                        .unwrap();

                    let entry_create_header = source_chain
                        .get_header(&original_header_address)
                        .unwrap()
                        .unwrap()
                        .clone();

                    let entry_update_hash = source_chain
                        .put(
                            builder::Update {
                                entry_type: eu_entry_type,
                                entry_hash: new_entry_hash,
                                original_header_address: original_header_address.clone(),
                                original_entry_address: original_entry_hash,
                            },
                            Some(new_entry),
                        )
                        .await
                        .unwrap();

                    let entry_update_header = source_chain
                        .get_header(&entry_update_hash)
                        .unwrap()
                        .unwrap()
                        .clone();

                    env_ref
                        .with_commit::<SourceChainError, _, _>(|writer| {
                            source_chain.flush_to_txn(writer)?;
                            Ok(())
                        })
                        .unwrap();
                    (entry_create_header, entry_update_header)
                };

                // Gather the expected op hashes, ops and basis
                // We are only expecting Store Element and Register Replaced By ops and nothing else
                let store_element_count = Arc::new(AtomicU32::new(0));
                let register_replaced_by_count = Arc::new(AtomicU32::new(0));
                let register_updated_element_count = Arc::new(AtomicU32::new(0));
                let register_agent_activity_count = Arc::new(AtomicU32::new(0));

                let expected = {
                    let mut map = HashMap::new();
                    // Op is expected to not contain the Entry even though the above contains the entry
                    let (entry_create_header, sig) =
                        entry_create_header.into_header_and_signature();
                    let expected_op = DhtOp::RegisterAgentActivity(
                        sig.clone(),
                        entry_create_header.clone().into_content(),
                    );
                    let op_hash = DhtOpHashed::from_content_sync(expected_op.clone()).into_hash();
                    map.insert(
                        op_hash,
                        (expected_op, register_agent_activity_count.clone()),
                    );

                    let expected_op = DhtOp::StoreElement(
                        sig,
                        entry_create_header.into_content().try_into().unwrap(),
                        None,
                    );
                    let op_hash = DhtOpHashed::from_content_sync(expected_op.clone()).into_hash();

                    map.insert(op_hash, (expected_op, store_element_count.clone()));

                    // Create RegisterUpdatedContent
                    // Op is expected to not contain the Entry
                    let (entry_update_header, sig) =
                        entry_update_header.into_header_and_signature();
                    let entry_update_header: Update =
                        entry_update_header.into_content().try_into().unwrap();
                    let expected_op =
                        DhtOp::StoreElement(sig.clone(), entry_update_header.clone().into(), None);
                    let op_hash = DhtOpHashed::from_content_sync(expected_op.clone()).into_hash();

                    map.insert(op_hash, (expected_op, store_element_count.clone()));

                    let expected_op = DhtOp::RegisterUpdatedContent(
                        sig.clone(),
                        entry_update_header.clone(),
                        None,
                    );
                    let op_hash = DhtOpHashed::from_content_sync(expected_op.clone()).into_hash();

                    map.insert(op_hash, (expected_op, register_replaced_by_count.clone()));
                    let expected_op = DhtOp::RegisterUpdatedElement(
                        sig.clone(),
                        entry_update_header.clone(),
                        None,
                    );
                    let op_hash = DhtOpHashed::from_content_sync(expected_op.clone()).into_hash();

                    map.insert(
                        op_hash,
                        (expected_op, register_updated_element_count.clone()),
                    );
                    let expected_op = DhtOp::RegisterAgentActivity(sig, entry_update_header.into());
                    let op_hash = DhtOpHashed::from_content_sync(expected_op.clone()).into_hash();
                    map.insert(
                        op_hash,
                        (expected_op, register_agent_activity_count.clone()),
                    );

                    map
                };

                // Create and fill authored ops db in the workspace
                {
                    let workspace = ProduceDhtOpsWorkspace::new(env.clone().into()).unwrap();
                    let (mut qt, _rx) = TriggerSender::new();
                    let complete = produce_dht_ops_workflow(workspace, env.clone().into(), &mut qt)
                        .await
                        .unwrap();
                    assert_matches!(complete, WorkComplete::Complete);
                }

                // Create cell data
                let dna = fixt!(DnaHash);
                let agents = AgentPubKeyFixturator::new(Unpredictable)
                    .take(num_agents as usize)
                    .collect::<Vec<_>>();

                // Create the network

                let filter_events = |evt: &_| match evt {
                    holochain_p2p::event::HolochainP2pEvent::Publish { .. } => true,
                    _ => false,
                };
                let (tx, mut recv) = tokio::sync::mpsc::channel(10);
                let test_network = test_network_with_events(
                    Some(dna.clone()),
                    Some(agents[0].clone()),
                    filter_events,
                    tx,
                )
                .await;
                let cell_network = test_network.cell_network();
                let (tx_complete, rx_complete) = tokio::sync::oneshot::channel();
                // We are expecting five ops per agent
                let total_expected = num_agents * 6;
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
                                    dht_hash,
                                    ops,
                                    ..
                                } => {
                                    tracing::debug!(?dht_hash);
                                    tracing::debug!(?ops);

                                    // Check the ops are correct
                                    for (op_hash, op) in ops {
                                        match expected.get(&op_hash) {
                                            Some((expected_op, count)) => {
                                                assert_eq!(&op, expected_op);
                                                assert_eq!(dht_hash, expected_op.dht_basis());
                                                count.fetch_add(1, Ordering::SeqCst);
                                            }
                                            None => panic!(
                                                "This DhtOpHash was not expected: {:?}",
                                                op_hash
                                            ),
                                        }
                                        recv_count += 1;
                                    }
                                    respond.respond(Ok(async move { Ok(()) }.boxed().into()));
                                    if recv_count == total_expected {
                                        tx_complete.take().unwrap().send(()).unwrap();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    .instrument(debug_span!("private_entries_inner"))
                });

                // Join some agents onto the network
                {
                    let network = test_network.network();
                    for agent in agents {
                        HolochainP2pRef::join(&network, dna.clone(), agent)
                            .await
                            .unwrap()
                    }
                }

                call_workflow(env.clone().into(), cell_network).await;

                // Wait for expected # of responses, or timeout
                tokio::select! {
                    _ = rx_complete => {}
                    _ = tokio::time::delay_for(RECV_TIMEOUT) => {
                        panic!("Timed out while waiting for expected responses.")
                    }
                };

                // Check there is no ops left that didn't come through
                assert_eq!(
                    num_agents * 1,
                    register_replaced_by_count.load(Ordering::SeqCst)
                );
                assert_eq!(
                    num_agents * 1,
                    register_updated_element_count.load(Ordering::SeqCst)
                );
                assert_eq!(num_agents * 2, store_element_count.load(Ordering::SeqCst));
                assert_eq!(
                    num_agents * 2,
                    register_agent_activity_count.load(Ordering::SeqCst)
                );

                // Shutdown
                tokio::time::timeout(Duration::from_secs(10), recv_task)
                    .await
                    .ok();
            }
            .instrument(debug_span!("private_entries")),
        );
    }

    // TODO: COVERAGE: Test public ops do publish
}
