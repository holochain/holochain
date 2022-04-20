#![deny(missing_docs)]
//! Proxy transport module for kitsune-p2p

use derive_more::*;
use futures::future::FutureExt;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::url2;
use kitsune_p2p_types::*;
use std::sync::Arc;

pub mod tx2;

mod proxy_url;
pub use proxy_url::*;
