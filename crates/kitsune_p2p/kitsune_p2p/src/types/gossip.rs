use crate::types::*;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::*;
use std::sync::Arc;

/// Represents an interchangeable gossip strategy module
pub trait AsGossipModule: 'static + Send + Sync {
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()>;
    fn local_agent_join(&self, a: Arc<KitsuneAgent>);
    fn local_agent_leave(&self, a: Arc<KitsuneAgent>);
}

pub struct GossipModule(pub Arc<dyn AsGossipModule>);

impl GossipModule {
    pub fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()> {
        self.0.incoming_gossip(con, gossip_data)
    }

    pub fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        self.0.local_agent_join(a);
    }

    pub fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        self.0.local_agent_leave(a);
    }
}

/// Represents an interchangeable gossip strategy module factory
pub trait AsGossipModuleFactory: 'static + Send + Sync {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule;
}

pub struct GossipModuleFactory(pub Arc<dyn AsGossipModuleFactory>);

impl GossipModuleFactory {
    pub fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule {
        self.0
            .spawn_gossip_task(tuning_params, space, ep_hnd, evt_sender)
    }
}
