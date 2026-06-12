//! Utilities for testing the consistency of the dht.

use crate::conductor::wire_rows_to_legacy_ops;
use holo_hash::AgentPubKey;
use holochain_state::dht_store::DhtStoreRead;
use holochain_state::prelude::*;
use kitsune2_api::OpId;

/// Request the published ops for the given agent from the DHT store.
///
/// Returns the ops `author` has authored and shares with the DHT — that is,
/// self-authored, locally-validated ops, with private `StoreEntry` ops
/// excluded so private entries never leak into the published set. Each op is
/// returned as `(loc, op_id, op)`, matching the legacy authored-db query
/// shape: `loc` is the basis location and `op_id` the located K2 op id.
pub async fn request_published_ops(
    dht_store: &DhtStoreRead,
    author: &AgentPubKey,
) -> StateQueryResult<Vec<(u32, OpId, DhtOp)>> {
    let chain = dht_store.published_chain_ops_for_wire(author).await?;
    Ok(wire_rows_to_legacy_ops(chain, Vec::new())
        .into_iter()
        .map(|op| {
            let basis = op.dht_basis();
            let hashed = DhtOpHashed::from_content_sync(op);
            (
                basis.get_loc(),
                hashed.hash.to_located_k2_op_id(&basis),
                hashed.content,
            )
        })
        .collect())
}
