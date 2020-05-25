#![deny(missing_docs)]
//! Types subcrate for kitsune-p2p.

/// Re-exported dependencies.
pub mod dependencies {
    pub use ::futures;
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

        /// Custom
        #[error("Custom: {0}")]
        Custom(Box<dyn std::error::Error + Send + Sync>),

        /// Other
        #[error("Other: {0}")]
        Other(String),
    }

    impl TransportError {
        /// promote a custom error type to a TransportError
        pub fn custom(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
            Self::Custom(e.into())
        }
    }

    impl From<String> for TransportError {
        fn from(s: String) -> Self {
            Self::Other(s)
        }
    }

    impl From<&str> for TransportError {
        fn from(s: &str) -> Self {
            s.to_string().into()
        }
    }

    /// Result type for remote communication.
    pub type TransportResult<T> = Result<T, TransportError>;

    /// Defines an established connection to a remote peer.
    pub mod transport_connection {
        ghost_actor::ghost_chan! {
            /// Event stream for handling incoming requests from a remote.
            pub chan TransportConnectionEvent<super::TransportError> {
                /// Event for handling incoming requests from a remote.
                fn incoming_request(url: url2::Url2, data: Vec<u8>) -> Vec<u8>;
            }
        }

        /// Receiver type for incoming connection events.
        pub type TransportConnectionEventReceiver =
            futures::channel::mpsc::Receiver<TransportConnectionEvent>;

        ghost_actor::ghost_actor! {
            /// Represents a connection to a remote node.
            pub actor TransportConnection<super::TransportError> {
                /// Retrieve the current url (address) of the remote end of this connection.
                fn remote_url() -> url2::Url2;

                /// Make a request of the remote end of this connection.
                fn request(data: Vec<u8>) -> Vec<u8>;
            }
        }
    }

    /// Defines a local binding
    /// (1) for accepting incoming connections and
    /// (2) for making outgoing connections.
    pub mod transport_listener {
        ghost_actor::ghost_chan! {
            /// Event stream for handling incoming connections.
            pub chan TransportListenerEvent<super::TransportError> {
                /// Event for handling incoming connections from a remote.
                fn incoming_connection(
                    sender: super::transport_connection::TransportConnectionSender,
                    receiver: super::transport_connection::TransportConnectionEventReceiver,
                ) -> ();
            }
        }

        /// Receiver type for incoming listener events.
        pub type TransportListenerEventReceiver =
            futures::channel::mpsc::Receiver<TransportListenerEvent>;

        ghost_actor::ghost_actor! {
            /// Represents a socket binding for establishing connections.
            pub actor TransportListener<super::TransportError> {
                /// Retrieve the current url (address) this listener is bound to.
                fn bound_url() -> url2::Url2;

                /// Attempt to establish an outgoing connection to a remote.
                fn connect(url: url2::Url2) -> (
                    super::transport_connection::TransportConnectionSender,
                    super::transport_connection::TransportConnectionEventReceiver,
                );
            }
        }
    }
}
