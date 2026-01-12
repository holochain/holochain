pub mod actor;
pub mod event;

pub(crate) mod wire;

pub use wire::WireDhtOpData;
pub use wire::WireMessage;

/// Error type for Holochain P2p.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HolochainP2pError {
    /// K2Error
    #[error(transparent)]
    K2Error(#[from] kitsune2_api::K2Error),

    /// RoutingDnaError
    #[error("Routing Dna Error: {0}")]
    RoutingDnaError(holo_hash::DnaHash),

    /// RoutingAgentError
    #[error("Routing Agent Error: {0}")]
    RoutingAgentError(holo_hash::AgentPubKey),

    /// Invalid P2p Message
    #[error("InvalidP2pMessage: {0}")]
    InvalidP2pMessage(String),

    /// No peers available for DHT location.
    ///
    /// This error is returned when there are no peers available for a given DHT location. If a
    /// p2p request is allowed to fail when no peers are available then this error can be used to
    /// filter the error from other network errors. For example, when getting links, and it is
    /// acceptable to return the links that are already held locally.
    #[error("{0}: No peers available for DHT location: {1}")]
    NoPeersForLocation(String, u32),

    /// K2 Space Not Found
    ///
    /// This error is returned when a p2p request tries to use a k2 space,
    /// but it does not exist.
    #[error("The K2 Space {0} does not exist")]
    K2SpaceNotFound(kitsune2_api::SpaceId),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),

    /// Chain Head Coordination error
    #[error(transparent)]
    ChcError(#[from] holochain_chc::ChcError),
}

/// Holochain p2p result type.
pub type HolochainP2pResult<T> = std::result::Result<T, HolochainP2pError>;

impl HolochainP2pError {
    /// promote a custom error type to a TransportError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }

    /// construct an invalid p2p message error variant
    pub fn invalid_p2p_message(s: String) -> Self {
        Self::InvalidP2pMessage(s)
    }
}

impl From<String> for HolochainP2pError {
    fn from(s: String) -> Self {
        #[derive(Debug, thiserror::Error)]
        struct OtherError(String);
        impl std::fmt::Display for OtherError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        HolochainP2pError::other(OtherError(s))
    }
}

impl From<&str> for HolochainP2pError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}
