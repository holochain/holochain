use crate::gossip::sharded_gossip::{BandwidthThrottle, GossipType, ShardedGossip};
use crate::test_util::spawn_handler;
use crate::types::gossip::*;
use crate::types::wire;
use futures::stream::StreamExt;
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

use super::switchboard_node::{SwitchboardEventHandler, SwitchboardNode};

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

/// An channel-based implementation of networking for tests, where messages are
/// simply routed in-memory
pub struct Switchboard {
    space: KSpace,
    agents: HashMap<Loc8, (AgentInfoSigned, SwitchboardNode)>,
    metric_tasks: Vec<JoinHandle<KitsuneResult<()>>>,
    handler_tasks: Vec<JoinHandle<ghost_actor::GhostResult<()>>>,
}

impl Switchboard {
    pub fn new() -> Self {
        Self {
            // TODO: randomize/parameterize space
            space: Arc::new(KitsuneSpace::new([0; 36].to_vec())),
            agents: Default::default(),
            metric_tasks: Default::default(),
            handler_tasks: Default::default(),
        }
    }

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

        let evt_handler = SwitchboardEventHandler::new(self.space.clone());
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
                old.0
            )
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
