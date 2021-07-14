#![deny(missing_docs)]
//! P2p / dht communication framework.

/// re-exported dependencies
pub mod dependencies {
    pub use ::kitsune_p2p_proxy;
    pub use ::kitsune_p2p_types;
    pub use ::url2;
}

mod types;
pub use types::*;

mod gossip;

mod config;
pub use config::*;

mod spawn;
pub use spawn::*;

#[cfg(test)]
pub mod test_util;

#[cfg(test)]
mod test;

pub mod fixt;
