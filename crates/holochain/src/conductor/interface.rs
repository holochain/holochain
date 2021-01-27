//! Interfaces are long-running tasks which listen for incoming messages
//! and dispatch them to the appropriate handlers within Holochain.
//! They also allow emitting responses and one-way Signals.
//!
//! Currently the only InterfaceDriver is a Websocket-based one, whose
//! implementation can be found in the `websocket` module here.

use crate::conductor::api::*;
use error::InterfaceError;
use error::InterfaceResult;
use holochain_types::signal::Signal;
use std::convert::TryInto;
use tokio::sync::broadcast;

#[allow(missing_docs)]
pub mod error;
pub mod websocket;

/// A collection of Senders to be used for emitting Signals from a Cell.
/// There is one Sender per attached Interface
#[derive(Clone, Debug)]
pub struct SignalBroadcaster {
    senders: Vec<broadcast::Sender<Signal>>,
}

impl SignalBroadcaster {
    /// send the signal to the connected client
    pub fn send(&mut self, sig: Signal) -> InterfaceResult<()> {
        self.senders
            .iter_mut()
            .map(|tx| tx.send(sig.clone()))
            .collect::<Result<Vec<_>, broadcast::SendError<Signal>>>()
            .map_err(InterfaceError::SignalSend)?;
        Ok(())
    }

    /// internal constructor
    pub fn new(senders: Vec<broadcast::Sender<Signal>>) -> Self {
        dbg!(&senders);
        Self { senders }
    }

    #[cfg(test)]
    /// A sender with nothing to send to. A placeholder for tests
    pub fn noop() -> Self {
        Self {
            senders: Vec::new(),
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Get a list of Signal receivers, one per sender (per interface)
    // NB: this could become more useful by giving identifiers to interfaces
    //     a returning a HashMap instead of a Vec
    pub fn subscribe_separately(&self) -> Vec<broadcast::Receiver<Signal>> {
        self.senders.iter().map(|s| s.subscribe()).collect()
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Get a single merged stream of all Signals from all broadcasters
    // NB: this could become more useful by giving identifiers to interfaces
    //     and returning tuples with keys instead of plain Signals
    pub fn subscribe_merged(&self) -> impl tokio::stream::Stream<Item = Signal> {
        use tokio::stream::StreamExt;
        let mut streams = tokio::stream::StreamMap::new();
        for (i, rx) in self.subscribe_separately().into_iter().enumerate() {
            streams.insert(i, rx);
        }
        streams.map(|(_, signal)| signal.expect("Signal channel closed"))
    }
}

pub use holochain_conductor_api::config::InterfaceDriver;
