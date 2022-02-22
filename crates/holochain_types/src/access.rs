//! Defines HostFnAccess and Permission

/// Access a call has to host functions
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct HostFnAccess {
    /// Can access agent information
    pub agent_info: Permission,
    /// Can access the workspace
    pub read_workspace: Permission,
    /// Can access the workspace deterministically.
    pub read_workspace_deterministic: Permission,
    /// Can write and workspace
    pub write_workspace: Permission,
    /// Can write to the network
    pub write_network: Permission,
    /// Can access bindings.
    pub bindings: Permission,
    /// Can access the deterministic bindings.
    pub bindings_deterministic: Permission,
    /// All other non-deterministic functions
    pub non_determinism: Permission,
    /// Access to functions that use the keystore in the conductor
    pub keystore: Permission,
    /// Access to deterministic keystore functions.
    pub keystore_deterministic: Permission,
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
    #[allow(clippy::too_many_arguments)]
    /// Constructor.
    pub fn new(
        agent_info: Permission,
        read_workspace: Permission,
        read_workspace_deterministic: Permission,
        write_workspace: Permission,
        write_network: Permission,
        bindings: Permission,
        bindings_deterministic: Permission,
        non_determinism: Permission,
        keystore: Permission,
        keystore_deterministic: Permission,
    ) -> Self {
        Self {
            agent_info,
            read_workspace,
            read_workspace_deterministic,
            write_workspace,
            write_network,
            bindings,
            bindings_deterministic,
            non_determinism,
            keystore,
            keystore_deterministic,
        }
    }
    /// Allow all access
    pub fn all() -> Self {
        HostFnAccess {
            read_workspace: Permission::Allow,
            read_workspace_deterministic: Permission::Allow,
            write_workspace: Permission::Allow,
            agent_info: Permission::Allow,
            non_determinism: Permission::Allow,
            write_network: Permission::Allow,
            keystore: Permission::Allow,
            keystore_deterministic: Permission::Allow,
            bindings: Permission::Allow,
            bindings_deterministic: Permission::Allow,
        }
    }

    /// Deny all access
    pub fn none() -> Self {
        HostFnAccess {
            read_workspace: Permission::Deny,
            read_workspace_deterministic: Permission::Deny,
            write_workspace: Permission::Deny,
            agent_info: Permission::Deny,
            non_determinism: Permission::Deny,
            write_network: Permission::Deny,
            keystore: Permission::Deny,
            keystore_deterministic: Permission::Deny,
            bindings: Permission::Deny,
            bindings_deterministic: Permission::Deny,
        }
    }
}
