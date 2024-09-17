use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;

use crate::prelude::*;

mod error;
pub use error::EventError;

mod op_event;
pub use op_event::OpEvent;

mod unsupported_event;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub timestamp: Timestamp,
    pub data: EventData,
}
impl Event {
    pub fn new(timestamp: Timestamp, data: impl Into<EventData>) -> Self {
        Self {
            timestamp,
            data: data.into(),
        }
    }
}

#[cfg(feature = "test_utils")]
impl Event {
    pub fn now(data: impl Into<EventData>) -> Self {
        Self {
            timestamp: Timestamp::now(),
            data: data.into(),
        }
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.timestamp.cmp(&other.timestamp) {
            std::cmp::Ordering::Equal => {
                self.data.ord_tiebreaker().cmp(other.data.ord_tiebreaker())
            }
            ordering => ordering,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, derive_more::From,
)]
pub enum EventData {
    Op(OpEvent),
}

impl EventData {
    /// Get some unique bytes for tiebreaking Ord in case timestamps are equal.
    fn ord_tiebreaker(&self) -> &[u8] {
        match self {
            Self::Op(e) => match e {
                OpEvent::Authored { op } => op.signature().as_ref(),
                OpEvent::Fetched { op } => op.signature().as_ref(),
                OpEvent::SysValidated { op } => op.as_ref(),
                OpEvent::AppValidated { op } => op.as_ref(),
                OpEvent::Integrated { op } => op.as_ref(),
            },
        }
    }
}

pub type EventResult<T> = Result<T, EventError>;
