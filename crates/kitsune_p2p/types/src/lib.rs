#![deny(missing_docs)]
//! Types subcrate for kitsune-p2p.

/// Re-exported dependencies.
pub mod dependencies {
    pub use ::ghost_actor;
    pub use ::thiserror;
    pub use ::tokio;
    pub use ::url2;
}

/// A collection of definitions related to remote communication.
pub mod transport {
    /// Error related to remote communication.
    #[derive(Debug, thiserror::Error)]
    pub enum TransportError {
        /// GhostError
        #[error(transparent)]
        GhostError(#[from] ghost_actor::GhostError),
    }

    /// Defines an established connection to a remote peer.
    pub mod transport_connection {
        ghost_actor::ghost_chan! {
            Visibility(pub),
            Name(TransportConnectionEvent),
            Error(super::TransportError),
            Api {
                IncomingRequest(
                    "Event for handling incoming requests from a remote.",
                    (url2::Url2, Vec<u8>),
                    Vec<u8>,
                ),
            }
        }

        /// Receiver type for incoming connection events.
        pub type TransportConnectionEventReceiver =
            tokio::sync::mpsc::Receiver<TransportConnectionEvent>;

        ghost_actor::ghost_actor! {
            Visibility(pub),
            Name(TransportConnection),
            Error(super::TransportError),
            Api {
                Request(
                    "Make a request of the remote end of this connection.",
                    Vec<u8>,
                    Vec<u8>,
                ),
            }
        }
    }

    /// Defines a local binding
    /// (1) for accepting incoming connections and
    /// (2) for making outgoing connections.
    pub mod transport_listener {
        ghost_actor::ghost_chan! {
            Visibility(pub),
            Name(TransportListenerEvent),
            Error(super::TransportError),
            Api {
                IncomingConnection(
                    "Event for handling incoming connections from a remote.",
                    (url2::Url2, super::transport_connection::TransportConnectionSender, super::transport_connection::TransportConnectionEventReceiver),
                    (),
                ),
            }
        }

        /// Receiver type for incoming listener events.
        pub type TransportListenerEventReceiver =
            tokio::sync::mpsc::Receiver<TransportListenerEvent>;

        ghost_actor::ghost_actor! {
            Visibility(pub),
            Name(TransportListener),
            Error(super::TransportError),
            Api {
                Connect(
                    "Attempt to establish an outgoing connection to a remote.",
                    url2::Url2,
                    super::transport_connection::TransportConnectionSender,
                ),
            }
        }
    }
}
