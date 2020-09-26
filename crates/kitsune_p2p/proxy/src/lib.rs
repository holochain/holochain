#![deny(missing_docs)]
//! Proxy transport module for kitsune-p2p

use derive_more::*;
use futures::future::FutureExt;
use ghost_actor::{dependencies::must_future::MustBoxFuture, GhostControlSender};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2},
    transport::{transport_connection::*, transport_listener::*, *},
};
use lair_keystore_api::actor::*;
use std::sync::Arc;

mod proxy_url;
pub use proxy_url::*;

pub mod wire;
pub(crate) use wire::*;

#[allow(dead_code)]
mod wire_read;
#[allow(dead_code)]
mod wire_write;

mod config;
pub use config::*;

mod inner_listen;
pub use inner_listen::*;

#[allow(dead_code)]
mod tls_con;
pub(crate) use tls_con::*;

#[allow(dead_code)]
mod inner_con;
//pub(crate) use inner_con::*;
