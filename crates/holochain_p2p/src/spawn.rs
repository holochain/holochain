use crate::actor::*;
use crate::event::*;

use super::*;

mod actor;

/// Spawn a new HolochainP2p actor.
/// Conductor will call this on initialization.
pub async fn spawn_holochain_p2p(
    db_peer_meta: DbWrite<DbKindPeerMetaStore>,
    db_op: DbWrite<DbKindDht>,
    handler: DynHcP2pHandler,
    lair_client: holochain_keystore::MetaLairClient,
    compat: NetworkCompatParams,
) -> HolochainP2pResult<DynHcP2p> {
    actor::HolochainP2pActor::create(db_peer_meta, db_op, handler, lair_client, compat).await
}

/// Some parameters used as part of a protocol compability check during tx5 preflight
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NetworkCompatParams {
    /// The UUID of the installed DPKI service.
    /// If the service is backed by a Dna, this is the core 32 bytes of the DnaHash.
    pub dpki_uuid: Option<[u8; 32]>,
}
