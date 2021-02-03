#![deny(missing_docs)]
//! Types subcrate for kitsune-p2p.

/// Re-exported dependencies.
pub mod dependencies {
    pub use ::futures;
    pub use ::ghost_actor;
    pub use ::paste;
    pub use ::serde;
    pub use ::serde_json;
    pub use ::spawn_pressure;
    pub use ::thiserror;
    pub use ::tokio;
    pub use ::url2;
}

pub mod async_lazy;
mod auto_stream_select;
pub use auto_stream_select::*;
pub mod codec;
pub mod dht_arc;
pub mod metrics;
pub mod transport;
pub mod transport_mem;
pub mod transport_pool;
