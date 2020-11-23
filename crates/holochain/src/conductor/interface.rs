//! Interfaces are long-running tasks which listen for incoming messages
//! and dispatch them to the appropriate handlers within Holochain.
//! They also allow emitting responses and one-way Signals.
//!
//! Currently the only InterfaceDriver is a Websocket-based one, whose
//! implementation can be found in the `websocket` module here.

use crate::{conductor::api::*, core::signal::Signal};
use error::{InterfaceError, InterfaceResult};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use tokio::sync::broadcast;

#[allow(missing_docs)]
pub mod error;
pub mod websocket;

/// A collection of Senders to be used for emitting Signals from a Cell.
/// Senders may have been attached one per interface, or have been added
/// standalone
#[derive(Clone, Debug)]
pub struct SignalBroadcaster(Vec<broadcast::Sender<Signal>>);

impl SignalBroadcaster {
    /// send the signal to the connected client
    pub fn send(&mut self, sig: Signal) -> InterfaceResult<()> {
        self.0
            .iter_mut()
            .map(|tx| tx.send(sig.clone()))
            .collect::<Result<Vec<_>, broadcast::SendError<Signal>>>()
            .map_err(InterfaceError::SignalSend)?;
        Ok(())
    }

    /// get the inner vec of senders
    pub fn into_inner(&self) -> Vec<broadcast::Sender<Signal>> {
      self.0.clone()
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
/// any change to the serialization strategy is a **breaking change**.
#[derive(Clone, Deserialize, Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InterfaceDriver {
    /// An interface implemented via Websockets
    Websocket {
        /// The port on which to establish the WebsocketListener
        port: u16,
    },
}
