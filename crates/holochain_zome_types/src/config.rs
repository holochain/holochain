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
    /// Default is set by Holochain, currently 20
    pub max_size: Option<u32>,
    /// Override the r2d2 connection pool's min_idle parameter
    /// Default: max_size
    pub min_idle: Option<u32>,
    /// Ovveride the r2d2 connection pool's max_lifetime parameter
    /// Default: 30 minutes,
    pub max_lifetime: Option<Duration>,
    /// Override the r2d2 connection pool's idle_timeout parameter
    /// Default: 10 minutes
    pub idle_timeout: Option<Duration>,
    /// Override the r2d2 connection pool's connection_timeout parameter
    /// Default: 30 seconds
    pub connection_timeout: Option<Duration>,
}
