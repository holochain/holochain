use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;

use crate::prelude::*;

mod op_event;
pub use op_event::OpEvent;

mod unsupported_event;

pub struct Event {
    pub data: EventData,
    pub timestamp: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EventData {
    Op(OpEvent),
}
