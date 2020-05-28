use crate::holo_hash_core::HoloHashCoreHash;

/// Error type for Holochain P2p.
#[derive(Debug, thiserror::Error)]
pub enum HolochainP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// KitsuneP2pError
    #[error(transparent)]
    KitsuneP2pError(#[from] kitsune_p2p::KitsuneP2pError),

    /// SerializedBytesError
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    /// Custom
    #[error("Custom: {0}")]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl HolochainP2pError {
    /// promote a custom error type to a TransportError
    pub fn custom(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Custom(e.into())
    }
}

pub mod actor;
pub mod event;

pub(crate) mod wire;

macro_rules! to_kitsune {
    ($($i:ident<$h:ty, $hc:ty> -> $k:ty,)*) => {
        $(
            pub(crate) trait $i: ::std::clone::Clone + Sized {
                fn into_kitsune(self) -> ::std::sync::Arc<$k>;
                fn to_kitsune(self) -> ::std::sync::Arc<$k> {
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

to_kitsune! {
    DnaHashExt<holo_hash::DnaHash, crate::holo_hash_core::DnaHash> -> kitsune_p2p::KitsuneSpace,
    AgentPubKeyExt<holo_hash::AgentPubKey, crate::holo_hash_core::AgentPubKey> -> kitsune_p2p::KitsuneAgent,
}
