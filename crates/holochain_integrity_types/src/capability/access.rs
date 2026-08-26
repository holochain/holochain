use crate::capability::CapGrant;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::{Deserialize, Serialize};

/// Represents an attempt to access capabilities.
///
/// Either a local agent is claiming to be the author of a source chain, and therefore gets
/// implicit access to its own zome calls. Or an agent is attempting an operation with a
/// [`CapGrant`].
///
/// In either case, Holochain checks the calling agent and requested capability against the
/// [`CapAccess`] instance to determine whether to allow access. If access is denied, an
/// unauthorized response is expected.
///
/// See [`CapAccess::is_valid_for_zome_call`] and [`CapAccess::is_valid_for_direct_signal`] to see
/// how these checks are made.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub enum CapAccess {
    /// Grants the capability of calling every extern to the calling agent, provided the calling
    /// agent is the local chain author.
    ///
    /// This is implicit zome call access only. Every other capability, including
    /// `Capability::DirectSignal`, requires an explicit grant even for the chain author.
    ///
    /// This grant is compared to the current `Entry::Agent` entry on the source chain.
    ChainAuthor(AgentPubKey),

    /// An agent is attempting to exercise a capability granted by a [`CapGrant`].
    ///
    /// The pubkey of the calling agent is secured by the cryptographic handshake at the network
    /// layer. Unless the grant is unrestricted, the caller must also provide a secret that we
    /// check for in a private entry in the local chain.
    RemoteAgent(Box<CapGrant>),
}

impl From<holo_hash::AgentPubKey> for CapAccess {
    fn from(agent_hash: holo_hash::AgentPubKey) -> Self {
        CapAccess::ChainAuthor(agent_hash)
    }
}
