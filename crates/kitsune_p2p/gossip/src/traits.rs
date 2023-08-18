use std::sync::Arc;

use kitsune_p2p_types::KAgent;

use crate::{error::GossipResult, PeerId};

pub trait NetCon: std::fmt::Debug {
    fn peer_id(&self) -> PeerId;
}

pub type ArcNetCon = Arc<dyn NetCon + Send + Sync + 'static>;

/// Represents an interchangeable gossip strategy module
pub trait AsGossipModule: 'static + Send + Sync {
    fn close(&self);
    fn incoming_gossip(
        &self,
        con: ArcNetCon,
        remote_url: String,
        gossip_data: Box<[u8]>,
    ) -> GossipResult<()>;
    fn local_agent_join(&self, a: KAgent);
    fn local_agent_leave(&self, a: KAgent);
    fn new_integrated_data(&self) {}
}
