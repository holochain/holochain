use crate::event::{QueryOpHashesEvt, TimeWindow};
use crate::gossip::sharded_gossip::{BandwidthThrottle, GossipType, ShardedGossip};
use crate::test_util::spawn_handler;
use crate::types::gossip::*;
use crate::types::wire;
use futures::stream::StreamExt;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::{AgentInfoInner, AgentInfoSigned};
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::dht_arc::loc8::Loc8;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArc, DhtLocation};
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;

use super::switchboard_evt_handler::SwitchboardEventHandler;
use super::switchboard_node::SwitchboardNode;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

/// The value of the SwitchboardSpace::agents hashmap
pub struct AgentEntry {
    /// The AgentInfoSigned for this agent
    pub info: AgentInfoSigned,
    /// The ops held by this agent.
    /// Other data for this op can be found in SwitchboardSpace::ops
    pub ops: HashMap<Loc8, AgentOpEntry>,
}

/// The value of the SwitchboardSpace::ops hashmap
pub struct OpEntry {
    /// Not strictly necessary as it can be computed from the Loc8 key, but here
    /// for convenience since there is no one-step way to go from Loc8 -> KitsuneOpHash
    pub hash: KOpHash,
    /// The opaque data for the op. Probably doesn't matter and can be removed.
    pub data: Vec<u8>,
    /// The timestamp associated with this op. Same for all agents, intrinsic to the
    /// op itself.
    pub timestamp: Timestamp,
}

pub struct AgentOpEntry {
    pub is_integrated: bool,
}

pub struct Switchboard {
    space: SwitchboardSpace,
}

impl Switchboard {
    pub fn new(space: Option<KSpace>) -> Self {
        // TODO: randomize/parameterize space
        let space = space.unwrap_or_else(|| Arc::new(KitsuneSpace::new([0; 36].to_vec())));
        Self {
            space: SwitchboardSpace::new(space),
        }
    }

    /// Get the space-specific switchboard at this space hash.
    /// NB: Currently only one space is supported. All other spaces will panic.
    pub fn space(&mut self, space: KSpace) -> &mut SwitchboardSpace {
        assert_eq!(self.space.space, space, "Got query for unexpected space");
        &mut self.space
    }
}

/// An channel-based implementation of networking for tests, where messages are
/// simply routed in-memory
pub struct SwitchboardSpace {
    space: KSpace,
    pub(super) agents: HashMap<Loc8, AgentEntry>,
    pub(super) ops: HashMap<Loc8, OpEntry>,
    metric_tasks: Vec<JoinHandle<KitsuneResult<()>>>,
    handler_tasks: Vec<JoinHandle<ghost_actor::GhostResult<()>>>,
}

impl SwitchboardSpace {
    pub fn new(space: KSpace) -> Self {
        Self {
            space,
            agents: Default::default(),
            ops: Default::default(),
            metric_tasks: Default::default(),
            handler_tasks: Default::default(),
        }
    }

    pub fn agent_by_loc8(&self, loc8: Loc8) -> Option<&AgentEntry> {
        self.agents.get(&loc8)
    }

    pub fn agent_by_hash(&self, hash: &KitsuneAgent) -> Option<&AgentEntry> {
        self.agent_by_loc8(hash.get_loc().into())
    }

    /// Set up a channel for a new node
    pub async fn add_node(&mut self, mem_config: MemConfig) -> SwitchboardNode {
        let f = tx2_mem_adapter(mem_config).await.unwrap();
        let f = tx2_pool_promote(f, Default::default());
        let f = tx2_api(f, Default::default());

        let mut ep = f
            .bind("none:", KitsuneTimeout::from_millis(5000))
            .await
            .unwrap();
        let ep_hnd = ep.handle().clone();

        let tuning_params = Arc::new(Default::default());

        let evt_handler = SwitchboardEventHandler::new(todo!(), todo!());
        let (evt_sender, task) = spawn_handler(evt_handler.clone()).await;

        self.handler_tasks.push(task);

        // TODO: generalize
        let gossip_type = GossipType::Historical;

        let bandwidth = Arc::new(BandwidthThrottle::new(1000.0, 1000.0));

        let gossip = ShardedGossip::new(
            tuning_params,
            self.space.clone(),
            ep_hnd.clone(),
            evt_sender,
            gossip_type,
            bandwidth,
        );

        let node = SwitchboardNode::new(evt_handler, GossipModule(gossip.clone()), ep_hnd);

        self.metric_tasks.push(metric_task(async move {
            dbg!("begin metric task");
            while let Some(evt) = ep.next().await {
                match dbg!(evt) {
                    // what other messages do i need to handle?
                    Tx2EpEvent::IncomingNotify(Tx2EpIncomingNotify { con, url, data, .. }) => {
                        match data {
                            wire::Wire::Gossip(wire::Gossip {
                                space: _,
                                data,
                                module,
                            }) => {
                                dbg!(&data, &module);
                                let data: Vec<u8> = data.into();
                                let data: Box<[u8]> = data.into_boxed_slice();

                                gossip.incoming_gossip(con, url, data).unwrap()
                            }
                            _ => unimplemented!(),
                        }
                    }
                    _ => unimplemented!(),
                }
            }
            Ok(())
        }));

        node
    }

    pub fn add_agent(
        &mut self,
        node: &SwitchboardNode,
        agent_loc8: Loc8,
        interval: ArcInterval<Loc8>,
    ) {
        let agent_loc: DhtLocation = agent_loc8.clone().into();
        let agent = Arc::new(KitsuneAgent::new(agent_loc.to_bytes_36()));
        let info = fake_agent_info(self.space.clone(), agent, interval.to_dht_location());
        if let Some(old) = self.agents.insert(agent_loc8, (info, node.clone())) {
            panic!(
                "Attempted to insert two agents at the same Loc8. Existing agent info: {:?}",
                old.info
            )
        }
    }

    pub fn query_op_hashes(
        &mut self,
        QueryOpHashesEvt {
            space,
            agents,
            window,
            max_ops,
            include_limbo,
        }: QueryOpHashesEvt,
    ) -> Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)> {
        let (ops, timestamps): (Vec<_>, Vec<_>) = self
            .ops
            .iter()
            .filter(|(op_loc8, op)| {
                // Does the op fall within the time window?
                window.contains(&op.timestamp)
                    // Does the op fall within one of the specified arcsets
                    // with the correct integration/limbo criteria?
                        && agents.into_iter().fold(false, |yes, (agent, arc_set)| {
                            if yes {
                                return true;
                            }
                            arc_set.contains((**op_loc8).into()) &&
                            self.agents
                                .get(&agent.get_loc().into())
                                .and_then(|agent| {
                                    agent
                                        .ops
                                        // Does agent hold this op?
                                        .get(op_loc8)
                                        // Does it meet the limbo criteria of the query?
                                        .map(|op| include_limbo || op.is_integrated)
                                })
                                .unwrap_or(false)
                        })
            })
            .map(|(_, op)| (op.hash, op.timestamp))
            .take(max_ops)
            .unzip();

        if ops.is_empty() {
            None
        } else {
            let window = timestamps
                .into_iter()
                .fold(window, |mut window, timestamp| {
                    if window.start < timestamp {
                        window.start = timestamp;
                    }
                    if window.end > timestamp {
                        window.end = timestamp;
                    }
                    window
                });
            Some((ops, window))
        }
    }
}

fn fake_agent_info(space: KSpace, agent: KAgent, interval: ArcInterval) -> AgentInfoSigned {
    let state = AgentInfoInner {
        space,
        agent,
        storage_arc: DhtArc::from_interval(interval),
        url_list: vec![],
        signed_at_ms: 0,
        expires_at_ms: 0,
        signature: Arc::new(KitsuneSignature(vec![])),
        encoded_bytes: Box::new([]),
    };
    AgentInfoSigned(Arc::new(state))
}
