//! Common config structs
//!
//! This only needs to be in this crate because the same struct is used across
//! both holochain_sqlite and holochain_conductor_api

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Modify behavior of the database connection pools
#[derive(Clone, Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct ConnectionPoolConfig {
    /// Override the r2d2 connection pool's max_size parameter
    pub max_size: Option<u32>,
    /// Override the r2d2 connection pool's min_idle parameter
    pub min_idle: Option<u32>,
    /// Override the r2d2 connection pool's idle_timeout parameter
    pub idle_timeout: Option<Duration>,
    /// Override the r2d2 connection pool's connection_timeout parameter
    pub connection_timeout: Option<Duration>,
}
