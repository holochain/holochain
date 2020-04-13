use serde::{self, Deserialize, Serialize};

/// Configure which app instance id to treat as the DPKI application handler
/// as well as what parameters to pass it on its initialization
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct DpkiConfig {
    /// TODO: DOCS: ?
    pub instance_id: String,
    /// TODO: DOCS: ?
    pub init_params: String,
}
