//! Types for Chain Head Coordination

use holochain_p2p::ChcImpl;
use holochain_zome_types::CellId;
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Arc};

mod chc_local;
pub use chc_local::*;

mod chc_remote;
pub use chc_remote::*;

static CHC_LOCAL_MAP: Lazy<parking_lot::Mutex<HashMap<CellId, Arc<ChcLocal>>>> =
    Lazy::new(|| parking_lot::Mutex::new(HashMap::new()));

/// Build the appropriate CHC implementation.
///
/// In particular, if the namespace is the magic string "#LOCAL#", then a [`ChcLocal`]
/// implementation will be used. Otherwise, if the namespace is set, and the CellId
/// is "CHC-enabled", then a [`ChcRemote`] will be produced.
pub fn build_chc(namespace: Option<&String>, cell_id: &CellId) -> Option<ChcImpl> {
    // TODO: check if the agent key is Holo-hosted, otherwise return none
    let is_holo_agent = true;
    if is_holo_agent {
        namespace.map(|ns| {
            if ns == "#LOCAL#" {
                chc_local(cell_id.clone())
            } else {
                chc_remote(ns, cell_id)
            }
        })
    } else {
        None
    }
}

fn chc_local(cell_id: CellId) -> ChcImpl {
    let mut m = CHC_LOCAL_MAP.lock();
    m.entry(cell_id)
        .or_insert_with(|| Arc::new(ChcLocal::new()))
        .clone()
}

fn chc_remote(namespace: &str, cell_id: &CellId) -> ChcImpl {
    Arc::new(ChcRemote::new(namespace, cell_id))
}
