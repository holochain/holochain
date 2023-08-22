use std::sync::Arc;

pub mod bloom;
pub mod codec;
pub mod error;
pub mod gossip_type;
pub mod metrics;

pub mod module;
pub mod mux;
pub mod round;

mod traits;
pub use traits::*;

pub type PeerId = Arc<[u8; 32]>;
