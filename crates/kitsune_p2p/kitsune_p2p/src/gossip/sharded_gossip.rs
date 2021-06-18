use crate::agent_store::AgentInfoSigned;
use crate::event::MetricQuery;
use crate::event::MetricQueryAnswer;
use crate::types::event::*;
use crate::types::gossip::*;
use crate::types::*;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
struct ShardedGossip {
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
            // tuning_params,
            // space,
            // ep_hnd,
            // send_interval_ms,
            // evt_sender,
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
        // TODO: Get remote endpoint
        // TODO: Send local agents
        // TODO: Wait for bandwidth 
        todo!();
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
