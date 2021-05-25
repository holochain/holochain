use holochain_zome_types::config::ConnectionPoolConfig;
use serde::{Deserialize, Serialize};

/// Config used for debugging and diagnosing issues with Holochain.
#[derive(Clone, Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct DevConfig {
    /// Config for database connection pools
    pub db_connection_pool: Option<ConnectionPoolConfig>,
}
