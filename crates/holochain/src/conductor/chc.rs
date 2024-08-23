//! Types for Chain Head Coordination

use holochain_keystore::MetaLairClient;
use holochain_p2p::ChcImpl;
use holochain_zome_types::prelude::*;
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Arc};
use url::Url;

mod chc_local;
pub use chc_local::*;

mod chc_remote;
pub use chc_remote::*;

/// Storage for the local CHC implementations
pub static CHC_LOCAL_MAP: Lazy<parking_lot::Mutex<HashMap<CellId, Arc<ChcLocal>>>> =
    Lazy::new(|| parking_lot::Mutex::new(HashMap::new()));

/// The URL which indicates that the fake local CHC service should be used,
/// instead of a remote service via HTTP
pub const CHC_LOCAL_MAGIC_URL: &str = "local:";

/// Build the appropriate CHC implementation.
///
/// In particular, if the url is the magic string "local:", then a [`ChcLocal`]
/// implementation will be used. Otherwise, if the url is set, and the CellId
/// is "CHC-enabled", then a [`ChcRemote`] will be produced.
pub fn build_chc(url: Option<&Url>, keystore: MetaLairClient, cell_id: &CellId) -> Option<ChcImpl> {
    // TODO: check if the agent key is Holo-hosted, otherwise return none
    let is_holo_agent = true;
    if is_holo_agent {
        url.map(|url| {
            if url.as_str() == CHC_LOCAL_MAGIC_URL {
                chc_local(keystore, cell_id.clone())
            } else {
                chc_remote(url.clone(), keystore, cell_id)
            }
        })
    } else {
        None
    }
}

fn chc_local(keystore: MetaLairClient, cell_id: CellId) -> ChcImpl {
    let agent = cell_id.agent_pubkey().clone();
    let mut m = CHC_LOCAL_MAP.lock();
    m.entry(cell_id)
        .or_insert_with(|| Arc::new(ChcLocal::new(keystore, agent)))
        .clone()
}

fn chc_remote(url: Url, keystore: MetaLairClient, cell_id: &CellId) -> ChcImpl {
    Arc::new(ChcRemote::new(url, keystore, cell_id))
}
