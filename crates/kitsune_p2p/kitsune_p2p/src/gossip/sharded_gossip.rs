use crate::agent_store::AgentInfoSigned;
use crate::event::MetricQuery;
use crate::event::MetricQueryAnswer;
use crate::types::event::*;
use crate::types::gossip::*;
use crate::types::*;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArcSet};
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
struct ShardedGossip {
    tuning_params: KitsuneP2pTuningParams,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    space: Arc<KitsuneSpace>,
    evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    inner: Share<ShardedGossipInner>,
}

struct ShardedGossipInner {
    local_agents: HashSet<Arc<KitsuneAgent>>,
}

impl ShardedGossip {
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Arc<Self> {
        let this = Arc::new(Self {
            tuning_params,
            space,
            ep_hnd,
            // send_interval_ms,
            evt_sender,
            inner: Share::new(ShardedGossipInner::new()),
        });
        metric_task({
            let this = this.clone();
            async move {
                loop {
                    this.run_one_iteration().await;
                }
                KitsuneResult::Ok(())
            }
        });
        this
    }

    async fn run_one_iteration(&self) -> () {
        // TODO: Calculate common arc set
        // TODO: Handle errors
        self.step_1_local_agents().await.unwrap();
        todo!()
    }

    async fn step_1_local_agents(&self) -> KitsuneResult<()> {
        // Get local agents
        let local_agents = self.inner.share_mut(|i, _| Ok(i.local_agents.clone()))?;
        let agent = local_agents.iter().cloned().next();

        let (since_ms, until_ms) = self.calculate_time_range()?;

        match agent {
            Some(agent) => {
                // TODO: Get all local agents and create an arc set
                let intervals: Vec<_> = self
                    .evt_sender
                    .query_agent_info_signed(QueryAgentInfoSignedEvt {
                        space: self.space.clone(),
                        agent: agent.clone(),
                    })
                    .await
                    // TODO: Handle error.
                    .unwrap()
                    .into_iter()
                    .filter(|info| local_agents.contains(info.agent.as_ref()))
                    .map(|info| info.storage_arc.interval())
                    .collect();
                let arc_set: DhtArcSet = intervals.into();
                let remote_agents: Vec<_> = self
                    .evt_sender
                    .query_gossip_agents(QueryGossipAgentsEvt {
                        space: self.space.clone(),
                        agent: agent.clone(),
                        since_ms,
                        until_ms,
                        arc_set: Arc::new(arc_set.clone()),
                    })
                    .await
                    // TODO: Handle error.
                    .unwrap()
                    .into_iter()
                    // Remove local agents.
                    .filter(|i| !local_agents.contains(&i.0))
                    .collect();

                // TODO: Get remote endpoint
                let endpoint = match remote_agents.into_iter().next() {
                    Some((remote_agent, _)) => {
                        let remote_agent = self
                            .evt_sender
                            .get_agent_info_signed(GetAgentInfoSignedEvt {
                                space: self.space.clone(),
                                agent: remote_agent.clone(),
                            })
                            .await
                            // TODO: Handle error.
                            .unwrap();
                        remote_agent.and_then(|ra| ra.url_list.get(0).cloned())
                    }
                    None => todo!(),
                };
                let timeout = self.tuning_params.implicit_timeout();
                let conn = match endpoint {
                    Some(url) => self.ep_hnd.get_connection(url, timeout).await?,
                    None => todo!(),
                };
                // Send local agents
                // TODO: Wait for bandwidth
                let gossip = ShardedGossipWire::initiate(local_agents.clone(), arc_set.intervals());
                let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
                let gossip = wire::Wire::gossip(self.space.clone(), gossip.into());
                conn.notify(&gossip, timeout).await?;
            }
            None => todo!(),
        }
        Ok(())
    }

    fn calculate_time_range(&self) -> KitsuneResult<(u64, u64)> {
        todo!()
    }
}

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec ShardedGossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            agents.0: HashSet<Arc<KitsuneAgent>>,
            arc_set.1: Vec<ArcInterval>,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            agents.0: HashMap<Arc<KitsuneAgent>, ArcInterval>,
        },

        /// Send a chunks of gossip meta op data,
        /// if "finished" this will be the final chunk.
        Chunk(0x30) {
            agents.0: HashSet<Arc<KitsuneAgent>>,
            // finished.1: bool,
            // chunks.2: Vec<Arc<MetaOpData>>,
        },
    }
}

impl ShardedGossipInner {
    fn new() -> Self {
        Self {
            local_agents: HashSet::new(),
        }
    }
}

impl AsGossipModule for ShardedGossip {
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()> {
        // use kitsune_p2p_types::codec::*;
        // let (_, gossip) = GossipWire::decode_ref(&gossip_data).map_err(KitsuneError::other)?;
        // self.inner.share_mut(move |i, _| {
        //     i.incoming.push((con, gossip));
        //     if i.incoming.len() > 20 {
        //         tracing::warn!(
        //             "Overloaded with incoming gossip.. {} messages",
        //             i.incoming.len()
        //         );
        //     }
        //     Ok(())
        // })
        todo!()
    }

    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.inner.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.inner.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }
}

struct ShardedGossipFactory;

impl AsGossipModuleFactory for ShardedGossipFactory {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule {
        GossipModule(ShardedGossip::new(tuning_params, space, ep_hnd, evt_sender))
    }
}
pub fn factory() -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(ShardedGossipFactory))
}
