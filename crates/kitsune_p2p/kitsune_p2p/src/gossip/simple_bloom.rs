use crate::agent_store::AgentInfoSigned;
use crate::types::gossip::*;
use crate::types::*;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MetaOpKey {
    Op(Arc<KitsuneOpHash>),
    Agent(Arc<KitsuneAgent>),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MetaOpData {
    Op(Arc<KitsuneOpHash>, PoolBuf),
    Agent(AgentInfoSigned),
}

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec GossipWire {
        /// Initiate
        Initiate(0x01) {
            filter.0: PoolBuf,
        },

        /// Accept
        Accept(0x02) {
            filter.0: PoolBuf,
        },

        /// Chunk
        Chunk(0x03) {
            finished.0: bool,
            chunks.1: Vec<MetaOpData>,
        },
    }
}

struct SimpleBloomModInner {
    _tuning_params: KitsuneP2pTuningParams,
    _space: Arc<KitsuneSpace>,
    _ep_hnd: Tx2EpHnd<wire::Wire>,
    _evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    local_agents: HashSet<Arc<KitsuneAgent>>,
}

impl SimpleBloomModInner {
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        Self {
            _tuning_params: tuning_params,
            _space: space,
            _ep_hnd: ep_hnd,
            _evt_sender: evt_sender,
            local_agents: HashSet::new(),
        }
    }
}

struct SimpleBloomMod(Share<SimpleBloomModInner>);

impl SimpleBloomMod {
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        let inner = SimpleBloomModInner::new(tuning_params, space, ep_hnd, evt_sender);
        SimpleBloomMod(Share::new(inner))
    }
}

impl AsGossipModule for SimpleBloomMod {
    fn incoming_gossip(&self, _gossip_data: Box<[u8]>) {}

    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }
}

struct SimpleBloomModFact;

impl AsGossipModuleFactory for SimpleBloomModFact {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule {
        GossipModule(Arc::new(SimpleBloomMod::new(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
        )))
    }
}

pub fn factory() -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(SimpleBloomModFact))
}
