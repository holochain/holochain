use crate::capability::CapGrant;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::{Deserialize, Serialize};

/// Represents an attempt to access capabilities.
///
/// Either an local agent is claiming to be the author of a source chain, and therefore gets
/// unrestricted access implicitly. Or a remote agent is attempting an operation with a
/// [`CapGrant`].
///
/// In either case, Holochain checks the calling agent and requested capability against the
/// [`CapAccess`] instance to determine whether to allow access. If access is denied, an
/// unauthorized response is expected.
///
/// See [`CapAccess::is_valid_for_zome_call`] to see how these checks are made.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub enum CapAccess {
    /// Grants the capability of calling every extern to the calling agent, provided the calling
    /// agent is the local chain author.
    ///
    /// This grant is compared to the current `Entry::Agent` entry on the source chain.
    ChainAuthor(AgentPubKey),

    /// Any agent other than the chain author is attempting to call an extern.
    ///
    /// The pubkey of the calling agent is secured by the cryptographic handshake at the network
    /// layer and the caller must provide a secret that we check for in a private entry in the
    /// local chain.
    RemoteAgent(CapGrant),
}

impl From<holo_hash::AgentPubKey> for CapAccess {
    fn from(agent_hash: holo_hash::AgentPubKey) -> Self {
        CapAccess::ChainAuthor(agent_hash)
    }
}
