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

/// Represents runtime data about an existing App interface.
/// Other stateful information like websocket ports can be found in
/// `ConductorState::app_interfaces`
pub enum AppInterfaceRuntime {
    /// A websocket app interface
    Websocket {
        /// The channel for this interface to send Signals across
        signal_tx: broadcast::Sender<Signal>,
    },

    #[cfg(any(test, feature = "test_utils"))]
    /// An interface used only for testing
    Test {
        /// The channel for this interface to send Signals across
        signal_tx: broadcast::Sender<Signal>,
    },
}

impl AppInterfaceRuntime {
    /// Get the signal sender for the interface
    pub fn signal_tx(&self) -> &broadcast::Sender<Signal> {
        match self {
            Self::Websocket { signal_tx, .. } => signal_tx,
            #[cfg(any(test, feature = "test_utils"))]
            Self::Test { signal_tx, .. } => signal_tx,
        }
    }
}

/// A collection of Senders to be used for emitting Signals from a Cell.
/// There is one Sender per attached Interface
#[derive(Clone, Debug)]
pub struct SignalBroadcaster {
    senders: Vec<broadcast::Sender<Signal>>,
}

impl SignalBroadcaster {
    /// send the signal to the connected client
    pub(crate) fn send(&mut self, sig: Signal) -> InterfaceResult<()> {
        self.senders
            .iter_mut()
            .map(|tx| tx.send(sig.clone()))
            .collect::<Result<Vec<_>, broadcast::error::SendError<Signal>>>()
            .map_err(InterfaceError::SignalSend)?;
        Ok(())
    }

    /// internal constructor
    pub fn new(senders: Vec<broadcast::Sender<Signal>>) -> Self {
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
    pub fn subscribe_merged(&self) -> impl tokio_stream::Stream<Item = Signal> {
        use tokio_stream::StreamExt;
        let mut streams = tokio_stream::StreamMap::new();
        for (i, rx) in self.subscribe_separately().into_iter().enumerate() {
            streams.insert(i, tokio_stream::wrappers::BroadcastStream::new(rx));
        }
        streams.map(|(_, signal)| signal.expect("Couldn't receive a signal"))
    }
}

pub use holochain_conductor_api::config::InterfaceDriver;
