use crate::holo_hash_core::HoloHashCoreHash;

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

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl HolochainP2pError {
    /// promote a custom error type to a TransportError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
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

pub mod actor;
pub mod event;

pub(crate) mod wire;

macro_rules! to_and_from_kitsune {
    ($($i:ident<$h:ty, $hc:ty,> -> $k:ty,)*) => {
        $(
            pub(crate) trait $i: ::std::clone::Clone + Sized {
                fn into_kitsune(self) -> ::std::sync::Arc<$k>;
                fn to_kitsune(&self) -> ::std::sync::Arc<$k> {
                    self.clone().into_kitsune()
                }
                fn from_kitsune(k: &::std::sync::Arc<$k>) -> Self;
            }

            impl $i for $h {
                fn into_kitsune(self) -> ::std::sync::Arc<$k> {
                    ::std::sync::Arc::new(self.into_inner().into())
                }

                fn from_kitsune(k: &::std::sync::Arc<$k>) -> Self {
                    <$hc>::new((**k).clone().into()).into()
                }
            }
        )*
    };
}

to_and_from_kitsune! {
    DnaHashExt<
        holo_hash::DnaHash,
        crate::holo_hash_core::DnaHash,
    > -> kitsune_p2p::KitsuneSpace,
    AgentPubKeyExt<
        holo_hash::AgentPubKey,
        crate::holo_hash_core::AgentPubKey,
    > -> kitsune_p2p::KitsuneAgent,
}

macro_rules! to_kitsune {
    ($($i:ident<$h:ty, $hc:ty,> -> $k:ty,)*) => {
        $(
            pub(crate) trait $i: ::std::clone::Clone + Sized {
                fn into_kitsune(self) -> ::std::sync::Arc<$k>;
                fn to_kitsune(&self) -> ::std::sync::Arc<$k> {
                    self.clone().into_kitsune()
                }
            }

            impl $i for $h {
                fn into_kitsune(self) -> ::std::sync::Arc<$k> {
                    ::std::sync::Arc::new(self.into_inner().into())
                }
            }
        )*
    };
}

to_kitsune! {
    AnyDhtHashExt<
        holochain_types::composite_hash::AnyDhtHash,
        crate::holo_hash_core::EntryContentHash,
    > -> kitsune_p2p::KitsuneBasis,
}
