//! holochain_types::dna::zome is a set of structs for working with holochain dna.

use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;

/// Represents an individual "zome".
#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, SerializedBytes)]
pub struct Zome {
    /// The WasmHash representing the WASM byte code for this zome.
    pub wasm_hash: holo_hash_core::WasmHash,
}

/// Access a call has to host functions
#[derive(Debug, Copy, Clone, Constructor)]
pub struct HostFnAccess {
    /// Can access agent information
    pub agent_info: Permission,
    /// Can access the workspace
    pub read_workspace: Permission,
    /// Can write to the network and workspace
    pub side_effects: Permission,
    /// All other non-deterministic functions
    pub non_determinism: Permission,
}

#[derive(Debug, Copy, Clone)]
/// Permission granted to a call
pub enum Permission {
    /// Host functions with this access will be included
    Allow,
    /// Host functions with this access will be unreachable
    Deny,
}

impl Zome {
    /// create a Zome from a holo_hash WasmHash instead of a holo_hash_core one
    pub fn from_hash(wasm_hash: holo_hash::WasmHash) -> Self {
        Self {
            wasm_hash: wasm_hash.into(),
        }
    }
}

impl Eq for Zome {}

impl HostFnAccess {
    /// Allow all access
    pub fn all() -> Self {
        HostFnAccess {
            read_workspace: Permission::Allow,
            side_effects: Permission::Allow,
            agent_info: Permission::Allow,
            non_determinism: Permission::Allow,
        }
    }

    /// Deny all access
    pub fn none() -> Self {
        HostFnAccess {
            read_workspace: Permission::Deny,
            side_effects: Permission::Deny,
            agent_info: Permission::Deny,
            non_determinism: Permission::Deny,
        }
    }
}
