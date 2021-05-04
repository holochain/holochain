use crate::*;

use kitsune_p2p_types::tx2::tx2_utils::*;
use types::direct::*;
use types::persist::*;

/// create a new v1 instance of the kitsune direct api
pub fn new_kitsune_direct_v1(
    // persistence module to use for this kdirect instance
    _persist: KdPersist,

    // v1 is only set up to run through a proxy
    // specify the proxy addy here
    _proxy: TxUrl,
) -> (
    KitsuneDirect,
    Box<dyn futures::Stream<Item = KitsuneDirectEvt>>,
) {
    unimplemented!()
}
