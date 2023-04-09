//! Kitsune P2p Direct Application Framework
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(unsafe_code)]

pub use kitsune_p2p_direct_api::{KdError, KdResult};
use kitsune_p2p_types::dependencies::ghost_actor::dependencies::tracing;
use kitsune_p2p_types::tx2::tx2_adapter::Uniq;
use kitsune_p2p_types::KitsuneError;

use std::sync::Arc;

pub mod types;

mod persist_mem;
pub use persist_mem::*;

mod srv;
pub use srv::*;

mod handle_ws;
pub use handle_ws::*;

mod v1;
pub use v1::*;

/// kdirect reexported dependencies
pub mod dependencies {
    pub use futures;
    pub use kitsune_p2p;
    pub use kitsune_p2p_types;
    pub use kitsune_p2p_types::dependencies::{
        ghost_actor::dependencies::tracing, holochain_trace,
    };
    pub use serde_json;
}

/// kdirect prelude
pub mod prelude {
    pub use crate::handle_ws::*;
    pub use crate::persist_mem::*;
    pub use crate::srv::*;
    pub use crate::types::direct::{KitsuneDirect, KitsuneDirectDriver};
    pub use crate::types::handle::{KdHnd, KdHndEvt, KdHndEvtStream};
    pub use crate::types::kdagent::{KdAgentInfo, KdAgentInfoExt};
    pub use crate::types::kdentry::{KdEntryContent, KdEntrySigned, KdEntrySignedExt};
    pub use crate::types::kdhash::{KdHash, KdHashExt};
    pub use crate::types::persist::KdPersist;
    pub use crate::types::srv::{HttpRespondCb, HttpResponse, KdSrv, KdSrvEvt, KdSrvEvtStream};
    pub use crate::v1::*;
    pub use kitsune_p2p::dht_arc::DhtArc;
    pub use kitsune_p2p_direct_api::{KdApi, KdError, KdResult};
}

use prelude::*;
