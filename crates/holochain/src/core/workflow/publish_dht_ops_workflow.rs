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
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holo_hash::*;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use std::collections::HashMap;
use std::time;
use tracing::*;

mod publish_query;

/// Default redundancy factor for validation receipts
pub const DEFAULT_RECEIPT_BUNDLE_SIZE: u8 = 5;

/// Don't publish a DhtOp more than once during this interval.
/// This allows us to trigger the publish workflow as often as we like, without
/// flooding the network with spurious publishes.
pub const MIN_PUBLISH_INTERVAL: time::Duration = time::Duration::from_secs(60 * 5);

#[instrument(skip(db, network, trigger_self))]
pub async fn publish_dht_ops_workflow(
    db: DbWrite<DbKindAuthored>,
    network: &(dyn HolochainP2pDnaT + Send + Sync),
    trigger_self: &TriggerSender,
    agent: AgentPubKey,
) -> WorkflowResult<WorkComplete> {
    let mut complete = WorkComplete::Complete;
    let to_publish = publish_dht_ops_workflow_inner(db.clone().into(), agent).await?;

    // Commit to the network
    tracing::info!("sending to {} basis locations", to_publish.len());
    let mut success = Vec::new();
    for (basis, ops) in to_publish {
        let (hashes, ops): (Vec<_>, Vec<_>) = ops.into_iter().unzip();
        if let Err(e) = network.publish(true, false, basis, ops, None).await {
            // If we get a routing error it means the space hasn't started yet and we should try publishing again.
            if let holochain_p2p::HolochainP2pError::RoutingDnaError(_) = e {
                complete = WorkComplete::Incomplete;
            }
            tracing::info!(failed_to_send_publish = ?e);
        } else {
            success.extend(hashes);
        }
    }

    tracing::info!("sent {} ops", success.len());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    let continue_publish = db
        .async_commit(move |writer| {
            for hash in success {
                mutations::set_last_publish_time(writer, &hash, now)?;
            }
            WorkflowResult::Ok(publish_query::num_still_needing_publish(writer)? > 0)
        })
        .await?;

    // If we have more ops that could be published then continue looping.
    if continue_publish {
        trigger_self.resume_loop();
    } else {
        trigger_self.pause_loop();
    }

    tracing::info!("committed sent ops");
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    Ok(complete)
}

/// Read the authored for ops with receipt count < R
pub async fn publish_dht_ops_workflow_inner(
    db: DbRead<DbKindAuthored>,
    agent: AgentPubKey,
) -> WorkflowResult<HashMap<AnyDhtHash, Vec<(DhtOpHash, DhtOp)>>> {
    // Ops to publish by basis
    let mut to_publish = HashMap::new();

    for op_hashed in publish_query::get_ops_to_publish(agent, &db).await? {
        let (op, op_hash) = op_hashed.into_inner();
        // For every op publish a request
        // Collect and sort ops by basis
        to_publish
            .entry(op.dht_basis())
            .or_insert_with(Vec::new)
            .push((op_hash, op));
    }

    Ok(to_publish)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixt::CreateLinkFixturator;
    use crate::fixt::EntryFixturator;
    use crate::test_utils::fake_genesis;
    use crate::test_utils::test_network_with_events;
    use crate::test_utils::TestNetwork;
    use ::fixt::prelude::*;
    use futures::future::FutureExt;
    use holochain_p2p::actor::HolochainP2pSender;
    use holochain_p2p::HolochainP2pDna;
    use holochain_p2p::HolochainP2pRef;
    use holochain_types::db_cache::DhtDbQueryCache;
    use observability;
    use rusqlite::Transaction;
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
    async fn setup(
        db: DbWrite<DbKindAuthored>,
        num_agents: u32,
        num_hash: u32,
        panic_on_publish: bool,
    ) -> (
        TestNetwork,
        HolochainP2pDna,
        AgentPubKey,
        JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        // Create data fixts for op
        let mut sig_fixt = SignatureFixturator::new(Unpredictable);
        let mut link_add_fixt = CreateLinkFixturator::new(Unpredictable);
        let author = fake_agent_pubkey_1();

        db.conn()
            .unwrap()
            .with_commit_sync(|txn| {
                for _ in 0..num_hash {
                    // Create data for op
                    let sig = sig_fixt.next().unwrap();
                    let mut link_add = link_add_fixt.next().unwrap();
                    link_add.author = author.clone();
                    // Create DhtOp
                    let op = DhtOp::RegisterAddLink(sig.clone(), link_add.clone());
                    // Get the hash from the op
                    let op_hashed = DhtOpHashed::from_content_sync(op.clone());
                    mutations::insert_op(txn, &op_hashed)?;
                }
                StateMutationResult::Ok(())
            })
            .unwrap();

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
        let test_network =
            test_network_with_events(Some(dna.clone()), Some(author.clone()), filter_events, tx)
                .await;
        let (tx_complete, rx_complete) = tokio::sync::oneshot::channel();
        let dna_network = test_network.dna_network();
        let network = test_network.network();
        let mut recv_count: u32 = 0;
        let total_expected = num_agents * num_hash;

        // Receive events and increment count
        let recv_task = tokio::task::spawn({
            async move {
                // use tokio_stream::StreamExt;
                let mut tx_complete = Some(tx_complete);
                while let Some(evt) = recv.recv().await {
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
            HolochainP2pRef::join(&network, dna.clone(), agent, None)
                .await
                .unwrap();
        }

        (test_network, dna_network, author, recv_task, rx_complete)
    }

    /// Call the workflow
    async fn call_workflow(
        db: DbWrite<DbKindAuthored>,
        dna_network: HolochainP2pDna,
        author: AgentPubKey,
    ) {
        let (trigger_sender, _) = TriggerSender::new();
        publish_dht_ops_workflow(db.clone().into(), &dna_network, &trigger_sender, author)
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
        tokio_helper::block_forever_on(async {
            observability::test_run().ok();

            // Create test db
            let test_db = test_authored_db();
            let db = test_db.to_db();

            // Setup
            let (_network, dna_network, author, recv_task, rx_complete) =
                setup(db.clone(), num_agents, num_hash, false).await;

            call_workflow(db.clone().into(), dna_network, author).await;

            // Wait for expected # of responses, or timeout
            tokio::select! {
                _ = rx_complete => {}
                _ = tokio::time::sleep(RECV_TIMEOUT) => {
                    panic!("Timed out while waiting for expected responses.")
                }
            };

            let check = async move {
                recv_task.await.unwrap();
                fresh_reader_test!(db, |txn: Transaction| {
                    let unpublished_ops: bool = txn
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE last_publish_time IS NULL)",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert!(!unpublished_ops);
                })
            };

            // Shutdown
            tokio::time::timeout(Duration::from_secs(10), check)
                .await
                .ok();
        });
    }

    /// There is a test that shows that if the receipt is set to complete
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
        tokio_helper::block_forever_on(async {
            observability::test_run().ok();

            // Create test db
            let test_db = test_authored_db();
            let db = test_db.to_db();

            // Setup
            let (_network, dna_network, author, _, _) =
                setup(db.clone(), num_agents, num_hash, true).await;

            // Update the authored to have complete receipts
            db.conn()
                .unwrap()
                .with_commit_test(|txn| {
                    txn.execute("UPDATE DhtOp SET receipts_complete = 1", [])
                        .unwrap();
                })
                .unwrap();

            // Call the workflow
            call_workflow(db.clone().into(), dna_network, author).await;

            // If we can wait a while without receiving any publish, we have succeeded
            tokio::time::sleep(Duration::from_millis(
                std::cmp::min(50, std::cmp::max(2000, 10 * num_agents * num_hash)).into(),
            ))
            .await;
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
        tokio_helper::block_forever_on(
            async {
                observability::test_run().ok();

                // Create test db
                let test_db = test_authored_db();
                let keystore = holochain_state::test_utils::test_keystore();
                let dht_db = test_dht_db();
                let db = test_db.to_db();
                let zome = fixt!(Zome);

                let dna = fixt!(DnaHash);
                let filter_events = |evt: &_| match evt {
                    holochain_p2p::event::HolochainP2pEvent::Publish { .. } => true,
                    _ => false,
                };
                let (tx, mut recv) = tokio::sync::mpsc::channel(10);
                let author = fake_agent_pubkey_1();
                let test_network = test_network_with_events(
                    Some(dna.clone()),
                    Some(author.clone()),
                    filter_events,
                    tx,
                )
                .await;
                let dna_network = test_network.dna_network();

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
                fake_genesis(db.clone(), dht_db.to_db(), keystore.clone())
                    .await
                    .unwrap();
                db.conn()
                    .unwrap()
                    .execute("UPDATE DhtOp SET receipts_complete = 1", [])
                    .unwrap();
                let author = fake_agent_pubkey_1();

                // Put data in elements
                let source_chain = SourceChain::new(
                    db.clone().into(),
                    dht_db.to_db(),
                    DhtDbQueryCache::new(dht_db.clone().into()),
                    keystore.clone(),
                    author.clone(),
                )
                .await
                .unwrap();
                // Produces 3 ops but minus 1 for store entry so 2 ops.
                let original_header_address = source_chain
                    .put(
                        Some(zome.clone()),
                        builder::Create {
                            entry_type: ec_entry_type,
                            entry_hash: original_entry_hash.clone(),
                        },
                        Some(original_entry),
                        ChainTopOrdering::default(),
                    )
                    .await
                    .unwrap();

                // Produces 5 ops but minus 1 for store entry so 4 ops.
                let entry_update_hash = source_chain
                    .put(
                        Some(zome.clone()),
                        builder::Update {
                            entry_type: eu_entry_type,
                            entry_hash: new_entry_hash,
                            original_header_address: original_header_address.clone(),
                            original_entry_address: original_entry_hash,
                        },
                        Some(new_entry),
                        ChainTopOrdering::default(),
                    )
                    .await
                    .unwrap();

                source_chain.flush(&dna_network).await.unwrap();
                let (entry_create_header, entry_update_header) = db
                    .conn()
                    .unwrap()
                    .with_commit_test(|writer| {
                        let store = Txn::from(writer);
                        let ech = store.get_header(&original_header_address).unwrap().unwrap();
                        let euh = store.get_header(&entry_update_hash).unwrap().unwrap();
                        (ech, euh)
                    })
                    .unwrap();

                // Gather the expected op hashes, ops and basis
                // We are only expecting Store Element and Register Replaced By ops and nothing else
                let store_element_count = Arc::new(AtomicU32::new(0));
                let register_replaced_by_count = Arc::new(AtomicU32::new(0));
                let register_updated_element_count = Arc::new(AtomicU32::new(0));
                let register_agent_activity_count = Arc::new(AtomicU32::new(0));

                let expected = {
                    let mut map = HashMap::new();
                    // Op is expected to not contain the Entry even though the above contains the entry
                    let (entry_create_header, sig) = entry_create_header.into_inner();
                    let expected_op = DhtOp::RegisterAgentActivity(
                        sig.clone(),
                        entry_create_header.clone().into_content(),
                    );
                    let op_hash = expected_op.to_hash();
                    map.insert(
                        op_hash,
                        (expected_op, register_agent_activity_count.clone()),
                    );

                    let expected_op = DhtOp::StoreElement(
                        sig,
                        entry_create_header.into_content().try_into().unwrap(),
                        None,
                    );
                    let op_hash = expected_op.to_hash();

                    map.insert(op_hash, (expected_op, store_element_count.clone()));

                    // Create RegisterUpdatedContent
                    // Op is expected to not contain the Entry
                    let (entry_update_header, sig) = entry_update_header.into_inner();
                    let entry_update_header: Update =
                        entry_update_header.into_content().try_into().unwrap();
                    let expected_op =
                        DhtOp::StoreElement(sig.clone(), entry_update_header.clone().into(), None);
                    let op_hash = expected_op.to_hash();

                    map.insert(op_hash, (expected_op, store_element_count.clone()));

                    let expected_op = DhtOp::RegisterUpdatedContent(
                        sig.clone(),
                        entry_update_header.clone(),
                        None,
                    );
                    let op_hash = expected_op.to_hash();

                    map.insert(op_hash, (expected_op, register_replaced_by_count.clone()));
                    let expected_op = DhtOp::RegisterUpdatedElement(
                        sig.clone(),
                        entry_update_header.clone(),
                        None,
                    );
                    let op_hash = expected_op.to_hash();

                    map.insert(
                        op_hash,
                        (expected_op, register_updated_element_count.clone()),
                    );
                    let expected_op = DhtOp::RegisterAgentActivity(sig, entry_update_header.into());
                    let op_hash = expected_op.to_hash();
                    map.insert(
                        op_hash,
                        (expected_op, register_agent_activity_count.clone()),
                    );

                    map
                };

                // Create cell data
                let agents = AgentPubKeyFixturator::new(Unpredictable)
                    .take(num_agents as usize)
                    .collect::<Vec<_>>();

                // Create the network

                let (tx_complete, rx_complete) = tokio::sync::oneshot::channel();
                // We are expecting six ops per agent plus one for self.
                // The 7 genesis ops were already recently published, so
                // won't be published again this time.
                let total_expected = (num_agents + 1) * 6;
                let mut recv_count: u32 = 0;

                // Receive events and increment count
                tokio::task::spawn({
                    async move {
                        let mut tx_complete = Some(tx_complete);
                        while let Some(evt) = recv.recv().await {
                            use holochain_p2p::event::HolochainP2pEvent::*;
                            match evt {
                                Publish { respond, ops, .. } => {
                                    tracing::debug!(?ops);

                                    // Check the ops are correct
                                    for op in ops {
                                        let op_hash = DhtOpHash::with_data_sync(&op);
                                        match expected.get(&op_hash) {
                                            Some((expected_op, count)) => {
                                                assert_eq!(&op, expected_op);
                                                count.fetch_add(1, Ordering::SeqCst);
                                            }
                                            None => {
                                                if let DhtOp::StoreEntry(_, h, _) = op {
                                                    if *h.visibility() == EntryVisibility::Private {
                                                        panic!(
                                                            "A private op has been published: {:?}",
                                                            h
                                                        )
                                                    }
                                                }
                                            }
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
                        HolochainP2pRef::join(&network, dna.clone(), agent, None)
                            .await
                            .unwrap()
                    }
                }

                call_workflow(db.clone().into(), dna_network, author).await;

                // Wait for expected # of responses, or timeout
                tokio::select! {
                    _ = rx_complete => {}
                    _ = tokio::time::sleep(RECV_TIMEOUT) => {
                        panic!("Timed out while waiting for expected responses.")
                    }
                };

                // We publish to ourself in a full sync network so we need
                // to expect one more op.
                let num_agents = num_agents + 1;
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
            }
            .instrument(debug_span!("private_entries")),
        );
    }
}
