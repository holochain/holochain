//! Kitsune P2p Direct Application Framework
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(unsafe_code)]

use kitsune_p2p_types::tx2::tx2_adapter::Uniq;
pub use kitsune_p2p_types::{KitsuneError, KitsuneResult};

use sodoken::Buffer;

use std::sync::Arc;

pub mod types;

mod persist_mem;
pub use persist_mem::*;

mod v1;
pub use v1::*;
