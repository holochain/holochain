use holochain_zome_types::config::ConnectionPoolConfig;
use serde::{Deserialize, Serialize};

/// Config used for debugging and diagnosing issues with Holochain.
#[derive(Clone, Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct DevConfig {
    db_connection_pool: Option<ConnectionPoolConfig>,
}
