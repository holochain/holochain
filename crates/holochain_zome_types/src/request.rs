//! Types for requesting metadata

use holochain_serialized_bytes::prelude::*;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
/// Metadata that can be requested on a basis
pub struct MetadataRequest {
    /// Get all the actions on an entry.
    /// Invalid request on a action.
    pub all_valid_actions: bool,
    // TODO: Implement after validation
    /// Placeholder
    pub all_invalid_actions: bool,
    /// Get all the deletes on a action
    pub all_deletes: bool,
    /// Get all the updates on an entry or action
    pub all_updates: bool,
    /// Placeholder
    pub follow_redirects: bool,
    /// Request the status of an entry.
    /// This is faster then getting all the actions
    /// and checking for live actions.
    pub entry_dht_status: bool,
}

impl Default for MetadataRequest {
    fn default() -> Self {
        Self {
            all_valid_actions: true,
            all_invalid_actions: false,
            all_deletes: true,
            all_updates: true,
            follow_redirects: false,
            entry_dht_status: false,
        }
    }
}
