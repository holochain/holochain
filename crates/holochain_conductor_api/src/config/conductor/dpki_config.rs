// Legacy config that will probably change
#![allow(missing_docs)]

use serde::Deserialize;
use serde::Serialize;

/// Configure which app instance ID to treat as the DPKI application handler
/// as well as what parameters to pass it on its initialization.
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub struct DpkiConfig {
    pub instance_id: String,
    pub init_params: String,
}
