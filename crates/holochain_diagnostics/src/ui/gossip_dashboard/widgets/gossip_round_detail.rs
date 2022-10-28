use std::fmt::Display;

use holochain::prelude::gossip::sharded_gossip::RoundThroughput;
use holochain::prelude::metrics::PeerNodeHistory;

use super::*;

pub struct GossipRoundDetailState<'a> {
    pub info: &'a PeerNodeHistory,
    pub start_time: Instant,
    pub current_time: Instant,
}

pub fn gossip_round_detail<Id: Display>(state: &GossipRoundDetailState) -> Table<'static> {
    todo!()
}
