/// Error type for Holochain P2p.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HolochainP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// RoutingDnaError
    #[error("Routing Dna Error: {0}")]
    RoutingDnaError(holo_hash::DnaHash),

    /// RoutingAgentError
    #[error("Routing Agent Error: {0}")]
    RoutingAgentError(holo_hash::AgentPubKey),

    /// OtherKitsuneP2pError
    #[error(transparent)]
    OtherKitsuneP2pError(kitsune_p2p::KitsuneP2pError),

    /// SerializedBytesError
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    /// Invalid P2p Message
    #[error("InvalidP2pMessage: {0}")]
    InvalidP2pMessage(String),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

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

// do some manual type translation so we get better error displays
impl From<kitsune_p2p::KitsuneP2pError> for HolochainP2pError {
    fn from(e: kitsune_p2p::KitsuneP2pError) -> Self {
        use kitsune_p2p::KitsuneP2pError::*;
        match e {
            RoutingSpaceError(space) => {
                Self::RoutingDnaError(holo_hash::DnaHash::from_kitsune(&space))
            }
            RoutingAgentError(agent) => {
                Self::RoutingAgentError(holo_hash::AgentPubKey::from_kitsune(&agent))
            }
            _ => Self::OtherKitsuneP2pError(e),
        }
    }
}

impl From<HolochainP2pError> for kitsune_p2p::KitsuneP2pError {
    fn from(e: HolochainP2pError) -> Self {
        use HolochainP2pError::*;
        match e {
            RoutingDnaError(dna) => Self::RoutingSpaceError(dna.to_kitsune()),
            RoutingAgentError(agent) => Self::RoutingAgentError(agent.to_kitsune()),
            OtherKitsuneP2pError(e) => e,
            _ => Self::other(e),
        }
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

/// Turn an [AgentKey] into a [KitsuneAgent]
pub fn agent_holo_to_kit(a: holo_hash::AgentPubKey) -> kitsune_p2p::KitsuneAgent {
    a.into_kitsune_raw()
}

/// Turn a [DnaHash] into a [KitsuneSpace]
pub fn space_holo_to_kit(d: holo_hash::DnaHash) -> kitsune_p2p::KitsuneSpace {
    d.into_kitsune_raw()
}

pub mod actor;
pub mod event;
pub mod mock_network;

pub(crate) mod wire;

macro_rules! to_and_from_kitsune {
    ($($i:ident<$h:ty> -> $k:ty,)*) => {
        $(
            /// Extension trait for holo/kitsune conversion
            pub trait $i: ::std::clone::Clone + Sized {
                /// convert into Arc<Kitsune> type
                fn into_kitsune(self) -> ::std::sync::Arc<$k>;

                /// convert into Kitsune type
                fn into_kitsune_raw(self) -> $k;

                /// to Arc<Kitsune> type
                fn to_kitsune(&self) -> ::std::sync::Arc<$k> {
                    self.clone().into_kitsune()
                }

                /// from Kitsune type
                fn from_kitsune(k: &::std::sync::Arc<$k>) -> Self;

                /// from Kitsune type
                fn from_kitsune_raw(k: $k) -> Self;
            }

            impl $i for $h {
                fn into_kitsune(self) -> ::std::sync::Arc<$k> {
                    ::std::sync::Arc::new(self.into_kitsune_raw())
                }

                fn into_kitsune_raw(self) -> $k {
                    <$k as kitsune_p2p::KitsuneBinType>::new(self.get_raw_36().to_vec())
                }

                fn from_kitsune(k: &::std::sync::Arc<$k>) -> Self {
                    <$h>::from_raw_36((**k).clone().into()).into()
                }

                fn from_kitsune_raw(k: $k) -> Self {
                    <$h>::from_raw_36(k.into()).into()
                }
            }
        )*
    };
}

to_and_from_kitsune! {
    DnaHashExt<holo_hash::DnaHash> -> kitsune_p2p::KitsuneSpace,
    AgentPubKeyExt<holo_hash::AgentPubKey> -> kitsune_p2p::KitsuneAgent,
    DhtOpHashExt<holo_hash::DhtOpHash> -> kitsune_p2p::KitsuneOpHash,
}

macro_rules! to_kitsune {
    ($($i:ident<$h:ty> -> $k:ty,)*) => {
        $(
            /// Extension trait for holo/kitsune conversion
            pub trait $i: ::std::clone::Clone + Sized {
                /// convert into Arc<Kitsune> type
                fn into_kitsune(self) -> ::std::sync::Arc<$k>;

                /// convert into Kitsune type
                fn into_kitsune_raw(self) -> $k;

                /// to Arc<Kitsune> type
                fn to_kitsune(&self) -> ::std::sync::Arc<$k> {
                    self.clone().into_kitsune()
                }
            }

            impl $i for $h {
                fn into_kitsune(self) -> ::std::sync::Arc<$k> {
                    ::std::sync::Arc::new(self.into_kitsune_raw())
                }

                fn into_kitsune_raw(self) -> $k {
                    <$k as kitsune_p2p::KitsuneBinType>::new(self.get_raw_36().to_vec())
                }
            }
        )*
    };
}

to_kitsune! {
    AnyDhtHashExt<holo_hash::AnyDhtHash> -> kitsune_p2p::KitsuneBasis,
}
