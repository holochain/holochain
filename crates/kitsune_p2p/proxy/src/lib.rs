#![deny(missing_docs)]
//! Proxy transport module for kitsune-p2p

use derive_more::*;
use kitsune_p2p_types::{dependencies::url2, transport::*};
use lair_keystore_api::actor::*;

mod proxy_url;
pub use proxy_url::*;
pub mod wire;
