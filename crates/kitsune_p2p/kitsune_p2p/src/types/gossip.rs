use crate::types::*;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::*;
use std::sync::Arc;

#[derive(Clone, Debug, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum GossipModuleType {
    Simple,
    ShardedRecent,
    ShardedHistorical,
}

/// Represents an interchangeable gossip strategy module
pub trait AsGossipModule: 'static + Send + Sync {
    fn close(&self);
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()>;
    fn local_agent_join(&self, a: Arc<KitsuneAgent>);
    fn local_agent_leave(&self, a: Arc<KitsuneAgent>);
    fn new_integrated_data(&self) {}
}

pub struct GossipModule(pub Arc<dyn AsGossipModule>);

impl GossipModule {
    pub fn close(&self) {
        self.0.close()
    }

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

    /// New data has been integrated and is ready for gossiping.
    pub fn new_integrated_data(&self) {
        self.0.new_integrated_data();
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

/// The specific provenance/destination of gossip is a particular Agent on
/// a connection specified by a Tx2Cert
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Constructor)]
pub struct GossipTgt {
    /// The agents on the remote node for whom this gossip is intended.
    /// In the current full-sync case, it makes sense to address gossip to all
    /// known agents on a node, but after sharding, we may make this a single
    /// agent target.
    pub agents: Vec<Arc<KitsuneAgent>>,
    /// The cert which represents the remote node to talk to.
    pub cert: Tx2Cert,
}

impl GossipTgt {
    /// Accessor
    pub fn agents(&self) -> &Vec<Arc<KitsuneAgent>> {
        &self.agents
    }

    /// Accessor
    pub fn cert(&self) -> &Tx2Cert {
        self.as_ref()
    }
}

impl AsRef<Tx2Cert> for GossipTgt {
    fn as_ref(&self) -> &Tx2Cert {
        &self.cert
    }
}
