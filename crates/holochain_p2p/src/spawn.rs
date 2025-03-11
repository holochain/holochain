use crate::actor::*;

use super::*;

mod actor;
pub use actor::WrapEvtSender;

/// Spawn a new HolochainP2p actor.
/// Conductor will call this on initialization.
pub async fn spawn_holochain_p2p(
    config: HolochainP2pConfig,
    lair_client: holochain_keystore::MetaLairClient,
) -> HolochainP2pResult<DynHcP2p> {
    actor::HolochainP2pActor::create(config, lair_client).await
}

/// Callback function to retrieve a peer meta database handle for a dna hash.
pub type GetDbPeerMeta = Arc<
    dyn Fn(DnaHash) -> BoxFut<'static, HolochainP2pResult<DbWrite<DbKindPeerMetaStore>>>
        + 'static
        + Send
        + Sync,
>;

/// Callback function to retrieve a op store database handle for a dna hash.
pub type GetDbOpStore = Arc<
    dyn Fn(DnaHash) -> BoxFut<'static, HolochainP2pResult<DbWrite<DbKindDht>>>
        + 'static
        + Send
        + Sync,
>;

/// HolochainP2p config struct.
pub struct HolochainP2pConfig {
    /// Callback function to retrieve a peer meta database handle for a dna hash.
    pub get_db_peer_meta: GetDbPeerMeta,

    /// Callback function to retrieve a op store database handle for a dna hash.
    pub get_db_op_store: GetDbOpStore,

    /// If true, will use kitsune core test bootstrap / transport / etc.
    pub k2_test_builder: bool,

    /// The compat params to use.
    pub compat: NetworkCompatParams,
}

impl Default for HolochainP2pConfig {
    fn default() -> Self {
        Self {
            get_db_peer_meta: Arc::new(|_| unimplemented!()),
            get_db_op_store: Arc::new(|_| unimplemented!()),
            k2_test_builder: false,
            compat: Default::default(),
        }
    }
}

/// Some parameters used as part of a protocol compability check during tx5 preflight
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NetworkCompatParams {
    /// The UUID of the installed DPKI service.
    /// If the service is backed by a Dna, this is the core 32 bytes of the DnaHash.
    pub dpki_uuid: Option<[u8; 32]>,
}
