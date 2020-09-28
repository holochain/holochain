use crate::{conductor::api::*, core::signal::Signal};
use error::{InterfaceError, InterfaceResult};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use tokio::sync::broadcast;

pub mod error;
pub mod websocket;

#[derive(Clone, Debug)]
pub struct SignalBroadcaster(Vec<broadcast::Sender<Signal>>);

impl SignalBroadcaster {
    /// send the signal to the connected client
    pub async fn send(&mut self, sig: Signal) -> InterfaceResult<()> {
        self.0
            .iter_mut()
            .map(|tx| tx.send(sig.clone()))
            .collect::<Result<Vec<_>, broadcast::SendError<Signal>>>()
            .map_err(InterfaceError::SignalSend)?;
        Ok(())
    }

    /// internal constructor
    pub fn new(senders: Vec<broadcast::Sender<Signal>>) -> Self {
        Self(senders)
    }

    #[cfg(test)]
    /// A sender with nothing to send to. A placeholder for tests
    pub fn noop() -> Self {
        Self(Vec::new())
    }
}

/// Configuration for interfaces, specifying the means by which an interface
/// should be opened.
///
/// NB: This struct is used in both [ConductorConfig] and [ConductorState], so
/// it is important that the serialization technique is not altered.
//
// TODO: write test that ensures the serialization is unaltered
#[derive(Clone, Deserialize, Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InterfaceDriver {
    Websocket { port: u16 },
}
