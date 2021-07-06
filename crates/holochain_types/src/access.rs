//! Defines HostFnAccess and Permission

/// Access a call has to host functions
#[derive(Debug, Copy, Clone, PartialEq, derive_more::Constructor)]
pub struct HostFnAccess {
    /// Can access agent information
    pub agent_info: Permission,
    /// Can access the workspace
    pub read_workspace: Permission,
    /// Can write and workspace
    pub write_workspace: Permission,
    /// Can write to the network
    pub write_network: Permission,
    /// Can access dna and zome specific data
    pub dna_bindings: Permission,
    /// All other non-deterministic functions
    pub non_determinism: Permission,
    /// Access to functions that use the keystore in the conductor
    pub keystore: Permission,
}

#[derive(Debug, Copy, Clone, PartialEq)]
/// Permission granted to a call
pub enum Permission {
    /// Host functions with this access will be included
    Allow,
    /// Host functions with this access will be unreachable
    Deny,
}

impl HostFnAccess {
    /// Allow all access
    pub fn all() -> Self {
        HostFnAccess {
            read_workspace: Permission::Allow,
            write_workspace: Permission::Allow,
            agent_info: Permission::Allow,
            non_determinism: Permission::Allow,
            write_network: Permission::Allow,
            keystore: Permission::Allow,
            dna_bindings: Permission::Allow,
        }
    }

    /// Deny all access
    pub fn none() -> Self {
        HostFnAccess {
            read_workspace: Permission::Deny,
            write_workspace: Permission::Deny,
            agent_info: Permission::Deny,
            non_determinism: Permission::Deny,
            write_network: Permission::Deny,
            keystore: Permission::Deny,
            dna_bindings: Permission::Deny,
        }
    }
}
