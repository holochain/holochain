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

/// See [NetworkCompatParams::proto_ver].
pub const HCP2P_PROTO_VER: u32 = 2;

/// Some parameters used as part of a protocol compability check during tx5 preflight
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct NetworkCompatParams {
    /// The current protocol version. This should be incremented whenever
    /// any breaking protocol changes are made to prevent incompatible
    /// nodes from talking to each other.
    pub proto_ver: u32,

    /// The UUID of the installed DPKI service.
    /// If the service is backed by a Dna, this is the core 32 bytes of the DnaHash.
    /// If not, set this to all zeroes.
    pub dpki_uuid: [u8; 32],
}

impl Default for NetworkCompatParams {
    fn default() -> Self {
        Self {
            proto_ver: HCP2P_PROTO_VER,
            dpki_uuid: [0; 32],
        }
    }
}
