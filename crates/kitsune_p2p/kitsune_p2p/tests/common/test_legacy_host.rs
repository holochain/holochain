use futures::{channel::mpsc::Receiver, FutureExt, StreamExt};
use itertools::Itertools;
use kitsune_p2p::event::{
    full_time_window, FetchOpDataEvt, FetchOpDataEvtQuery, KitsuneP2pEvent, PutAgentInfoSignedEvt,
    QueryAgentsEvt, QueryOpHashesEvt, SignNetworkDataEvt,
};
use kitsune_p2p_bin_data::{KitsuneAgent, KitsuneOpData, KitsuneSignature, KitsuneSpace};
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    dependencies::lair_keystore_api::LairClient,
    dht::{arq::LocalStorageConfig, spacetime::*, ArqStrat, PeerStrat},
    dht_arc::{DhtArc, DhtArcRange},
    KAgent,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use super::TestHostOp;

pub struct TestLegacyHost {
    handle: Option<tokio::task::JoinHandle<()>>,
    keystore: LairClient,
    events: Arc<futures::lock::Mutex<Vec<RecordedKitsuneP2pEvent>>>,
    duplicate_ops_received_count: Arc<AtomicU32>,
}

impl TestLegacyHost {
    pub fn new(keystore: LairClient) -> Self {
        let events = Arc::new(futures::lock::Mutex::new(Vec::new()));
        let duplicate_ops_received_count = Arc::new(AtomicU32::new(0));

        Self {
            handle: None,
            keystore,
            events,
            duplicate_ops_received_count,
        }
    }

    pub async fn start(
        &mut self,
        agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>,
        op_store: Arc<parking_lot::RwLock<Vec<TestHostOp>>>,
        receivers: Vec<Receiver<KitsuneP2pEvent>>,
    ) {
        if self.handle.is_some() {
            panic!("TestLegacyHost already started");
        }

        let handle = tokio::task::spawn({
            let keystore = self.keystore.clone();
            let events_record = self.events.clone();
            let duplicate_ops_received_count = self.duplicate_ops_received_count.clone();
            async move {
                let mut receiver = futures::stream::select_all(receivers).fuse();
                while let Some(evt) = receiver.next().await {
                    record_event(events_record.clone(), &evt).await;
                    match evt {
                        KitsuneP2pEvent::PutAgentInfoSigned { respond, input, .. } => {
                            let mut store = agent_store.write();
                            let incoming_agents: HashSet<_> =
                                input.peer_data.iter().map(|p| p.agent.clone()).collect();
                            store.retain(|p: &AgentInfoSigned| !incoming_agents.contains(&p.agent));
                            store.extend(input.peer_data);
                            respond.respond(Ok(async move { Ok(()) }.boxed().into()))
                        }
                        KitsuneP2pEvent::QueryAgents { respond, input, .. } => {
                            let kitsune_p2p::event::QueryAgentsEvt {
                                space,
                                agents,
                                window,
                                arc_set,
                                near_basis,
                                limit,
                            } = input;

                            let store = agent_store.read();

                            let agents = match (agents, window, arc_set, near_basis, limit) {
                                // Handle as a "near basis" query.
                                (None, None, None, Some(basis), Some(limit)) => {
                                    let mut out: Vec<(u32, &AgentInfoSigned)> = store
                                        .iter()
                                        .filter_map(|v| {
                                            if v.is_active() {
                                                Some((v.storage_arc.dist(basis.as_u32()), v))
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();

                                    out.sort_by(|a, b| a.0.cmp(&b.0));

                                    out
                                        .into_iter()
                                        .take(limit as usize)
                                        .map(|(_, v)| v.clone())
                                        .collect()
                                }

                                // Handle as a "gossip agents" query.
                                (_agents, window, Some(arc_set), None, None) => {
                                    let window = window.unwrap_or_else(full_time_window);
                                    let since_ms = window.start.as_millis().max(0) as u64;
                                    let until_ms = window.end.as_millis().max(0) as u64;

                                    store.iter().filter_map(|info| {
                                        if !info.is_active() {
                                            return None;
                                        }

                                        if info.signed_at_ms < since_ms {
                                            return None;
                                        }

                                        if info.signed_at_ms > until_ms {
                                            return None;
                                        }

                                        let interval = DhtArcRange::from(info.storage_arc);
                                        if !arc_set.overlap(&interval.into()) {
                                            return None;
                                        }

                                        Some(info.clone())
                                    })
                                    .collect()
                                }

                                // Otherwise, do a simple agent query with optional agent filter
                                (agents, None, None, None, None) => {
                                    match agents {
                                        Some(agents) => store
                                            .iter()
                                            .filter(|p| {
                                                p.space == space
                                                    && agents.contains(&p.agent)
                                            })
                                            .cloned()
                                            .collect::<Vec<_>>(),
                                        None => store.iter().cloned().collect(),
                                    }
                                }

                                // If none of the above match, we have no implementation for such a query
                                // and must fail
                                tuple => unimplemented!(
                                    "Holochain cannot interpret the QueryAgentsEvt data as given: {:?}",
                                    tuple
                                ),
                            };

                            respond.respond(Ok(async move { Ok(agents) }.boxed().into()))
                        }
                        KitsuneP2pEvent::QueryPeerDensity {
                            respond,
                            space,
                            dht_arc,
                            ..
                        } => {
                            let cutoff = std::time::Duration::from_secs(60 * 15);
                            let topology = Topology {
                                space: SpaceDimension::standard(),
                                time: TimeDimension::new(std::time::Duration::from_secs(60 * 5)),
                                time_origin: Timestamp::now(),
                                time_cutoff: cutoff,
                            };
                            let now = Timestamp::now().as_millis() as u64;
                            let arcs = agent_store
                                .read()
                                .iter()
                                .filter_map(|agent: &AgentInfoSigned| {
                                    if agent.space == space && now < agent.expires_at_ms {
                                        Some(agent.storage_arc)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>();

                            let strat = PeerStrat::Quantized(ArqStrat::standard(
                                LocalStorageConfig::default(),
                            ));
                            let view = strat.view(topology, dht_arc, &arcs);

                            respond.respond(Ok(async move { Ok(view) }.boxed().into()))
                        }
                        KitsuneP2pEvent::Call {
                            respond, payload, ..
                        } => {
                            // Echo the request payload
                            respond.respond(Ok(async move { Ok(payload) }.boxed().into()))
                        }
                        KitsuneP2pEvent::ReceiveOps { respond, ops, .. } => {
                            let mut op_store = op_store.write();
                            for op in ops {
                                let incoming_op: TestHostOp = op.clone().into();
                                if op_store.iter().any(|existing_op| {
                                    existing_op.kitsune_hash() == incoming_op.kitsune_hash()
                                }) {
                                    duplicate_ops_received_count.fetch_add(1, Ordering::Acquire);
                                    continue;
                                }
                                op_store.push(op.into());
                            }
                            respond.respond(Ok(async move { Ok(()) }.boxed().into()))
                        }
                        KitsuneP2pEvent::QueryOpHashes { respond, input, .. } => {
                            tracing::info!("QueryOpHashes: {:?}", input);
                            let op_store = op_store.read();
                            let selected_ops: Vec<TestHostOp> = op_store
                                .iter()
                                .filter(|op| {
                                    if op.space() != input.space {
                                        return false;
                                    }

                                    if op.authored_at() < input.window.start
                                        || op.authored_at() >= input.window.end
                                    {
                                        return false;
                                    }

                                    let intervals = input.arc_set.intervals();
                                    if let Some(DhtArcRange::Full) = intervals.first() {
                                        // Keep everything
                                    } else {
                                        let mut in_any = false;
                                        for interval in intervals {
                                            match interval {
                                                DhtArcRange::Bounded(lower, upper) => {
                                                    if lower < op.location()
                                                        && op.location() < upper
                                                    {
                                                        in_any = true;
                                                        break;
                                                    }
                                                }
                                                _ => unreachable!(
                                                    "Invalid input to host query for op hashes"
                                                ),
                                            }
                                        }

                                        if !in_any {
                                            return false;
                                        }
                                    }

                                    true
                                })
                                .sorted_by_key(|op| op.authored_at())
                                .take(input.max_ops)
                                .cloned()
                                .collect();

                            if !selected_ops.is_empty() {
                                let low_time = selected_ops.first().unwrap().authored_at();
                                let high_time = selected_ops.last().unwrap().authored_at();

                                respond.respond(Ok(async move {
                                    Ok(Some((
                                        selected_ops
                                            .into_iter()
                                            .map(|op| Arc::new(op.kitsune_hash()))
                                            .collect(),
                                        low_time..=high_time,
                                    )))
                                }
                                .boxed()
                                .into()))
                            } else {
                                respond.respond(Ok(async move { Ok(None) }.boxed().into()))
                            }
                        }
                        KitsuneP2pEvent::FetchOpData { respond, input, .. } => {
                            let result = match input.query {
                                FetchOpDataEvtQuery::Hashes { op_hash_list, .. } => {
                                    let search_hashes =
                                        op_hash_list.into_iter().collect::<HashSet<_>>();
                                    let op_store = op_store.read();
                                    let matched_host_data = op_store.iter().filter(|op| {
                                        op.space() == input.space
                                            && search_hashes.contains(&op.kitsune_hash())
                                    });

                                    matched_host_data
                                        .map(|h| (Arc::new(h.kitsune_hash()), h.clone().into()))
                                        .collect()
                                }
                                _ => {
                                    unimplemented!("Only know how to handle Hashes variant");
                                }
                            };

                            respond.respond(Ok(async move { Ok(result) }.boxed().into()))
                        }
                        KitsuneP2pEvent::SignNetworkData { respond, input, .. } => {
                            let mut key = [0; 32];
                            key.copy_from_slice(input.agent.0.as_slice());
                            let sig = keystore
                                .sign_by_pub_key(
                                    key.into(),
                                    None,
                                    input.data.as_slice().to_vec().into(),
                                )
                                .await
                                .unwrap();
                            respond.respond(Ok(async move { Ok(KitsuneSignature(sig.0.to_vec())) }
                                .boxed()
                                .into()))
                        }
                        _ => todo!("Unhandled event {:?}", evt),
                    }
                }
            }
        });

        self.handle = Some(handle);
    }

    pub async fn drain_events(&self) -> Vec<RecordedKitsuneP2pEvent> {
        let mut events = self.events.lock().await;
        std::mem::take(&mut *events)
    }

    #[allow(dead_code)]
    pub fn duplicate_ops_received_count(&self) -> u32 {
        self.duplicate_ops_received_count.load(Ordering::Acquire)
    }

    pub async fn create_agent(&self) -> KAgent {
        let tag = nanoid::nanoid!();
        let info = self
            .keystore
            .new_seed(tag.into(), None, false)
            .await
            .unwrap();
        Arc::new(KitsuneAgent(info.ed25519_pub_key.0.to_vec()))
    }
}

/// For recording events being received by the legacy host. This enum should match KitsuneP2pEvent with
/// the responders and tracing context removed. Just the payload should be available for test assertions.
pub enum RecordedKitsuneP2pEvent {
    PutAgentInfoSigned {
        input: PutAgentInfoSignedEvt,
    },
    QueryAgents {
        input: QueryAgentsEvt,
    },
    QueryPeerDensity {
        space: Arc<KitsuneSpace>,
        dht_arc: DhtArc,
    },
    Call {
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    },
    Notify {
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    },
    ReceiveOps {
        space: Arc<KitsuneSpace>,
        ops: Vec<Arc<KitsuneOpData>>,
        context: Option<FetchContext>,
    },
    QueryOpHashes {
        input: QueryOpHashesEvt,
    },
    FetchOpData {
        input: FetchOpDataEvt,
    },
    SignNetworkData {
        input: SignNetworkDataEvt,
    },
}

async fn record_event(
    events: Arc<futures::lock::Mutex<Vec<RecordedKitsuneP2pEvent>>>,
    evt: &KitsuneP2pEvent,
) {
    if events.lock().await.len() % 500 == 0 {
        dump_event_dist(events.clone()).await;
    }

    let mut events = events.lock().await;

    match evt {
        KitsuneP2pEvent::PutAgentInfoSigned { input, .. } => {
            events.push(RecordedKitsuneP2pEvent::PutAgentInfoSigned {
                input: input.clone(),
            });
        }
        KitsuneP2pEvent::QueryAgents { input, .. } => {
            events.push(RecordedKitsuneP2pEvent::QueryAgents {
                input: input.clone(),
            });
        }
        KitsuneP2pEvent::QueryPeerDensity { space, dht_arc, .. } => {
            events.push(RecordedKitsuneP2pEvent::QueryPeerDensity {
                space: space.clone(),
                dht_arc: *dht_arc,
            });
        }
        KitsuneP2pEvent::Call {
            space,
            to_agent,
            payload,
            ..
        } => {
            events.push(RecordedKitsuneP2pEvent::Call {
                space: space.clone(),
                to_agent: to_agent.clone(),
                payload: payload.clone(),
            });
        }
        KitsuneP2pEvent::Notify {
            space,
            to_agent,
            payload,
            ..
        } => {
            events.push(RecordedKitsuneP2pEvent::Notify {
                space: space.clone(),
                to_agent: to_agent.clone(),
                payload: payload.clone(),
            });
        }
        KitsuneP2pEvent::ReceiveOps {
            space,
            ops,
            context,
            ..
        } => {
            events.push(RecordedKitsuneP2pEvent::ReceiveOps {
                space: space.clone(),
                ops: ops.clone(),
                context: *context,
            });
        }
        KitsuneP2pEvent::QueryOpHashes { input, .. } => {
            events.push(RecordedKitsuneP2pEvent::QueryOpHashes {
                input: input.clone(),
            });
        }
        KitsuneP2pEvent::FetchOpData { input, .. } => {
            events.push(RecordedKitsuneP2pEvent::FetchOpData {
                input: input.clone(),
            });
        }
        KitsuneP2pEvent::SignNetworkData { input, .. } => {
            events.push(RecordedKitsuneP2pEvent::SignNetworkData {
                input: input.clone(),
            });
        }
    }
}

async fn dump_event_dist(events: Arc<futures::lock::Mutex<Vec<RecordedKitsuneP2pEvent>>>) {
    let events = events.lock().await;
    let mut counts = HashMap::new();
    for evt in events.iter() {
        let key = match evt {
            RecordedKitsuneP2pEvent::PutAgentInfoSigned { .. } => "PutAgentInfoSigned",
            RecordedKitsuneP2pEvent::QueryAgents { .. } => "QueryAgents",
            RecordedKitsuneP2pEvent::QueryPeerDensity { .. } => "QueryPeerDensity",
            RecordedKitsuneP2pEvent::Call { .. } => "Call",
            RecordedKitsuneP2pEvent::Notify { .. } => "Notify",
            RecordedKitsuneP2pEvent::ReceiveOps { .. } => "ReceiveOps",
            RecordedKitsuneP2pEvent::QueryOpHashes { .. } => "QueryOpHashes",
            RecordedKitsuneP2pEvent::FetchOpData { .. } => "FetchOpData",
            RecordedKitsuneP2pEvent::SignNetworkData { .. } => "SignNetworkData",
        };
        let count = counts.entry(key).or_insert(0);
        *count += 1;
    }
    tracing::info!("Events: {}, dist: {:?}", events.len(), counts);
}
