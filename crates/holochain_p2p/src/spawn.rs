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
    tracing::info!(?config, "Launching HolochainP2p");
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

    /// Callback function to retrieve an op store database handle for a dna hash.
    pub get_db_op_store: GetDbOpStore,

    /// The arc factor to apply to target arc hints.
    pub target_arc_factor: u32,

    /// Authentication material if required by sbd/signal/bootstrap services.
    pub auth_material: Option<Vec<u8>>,

    /// Configuration to pass to Kitsune2.
    ///
    /// This should contain module configurations such as [CoreBootstrapModConfig](kitsune2_core::factories::CoreBootstrapModConfig).
    pub network_config: Option<serde_json::Value>,

    /// The compat params to use.
    pub compat: NetworkCompatParams,

    /// If true, will use kitsune core test bootstrap / transport / etc.
    #[cfg(feature = "test_utils")]
    pub k2_test_builder: bool,

    /// If true, will disable the default bootstrap module.
    ///
    /// This flag is only used when [HolochainP2pConfig::k2_test_builder] is true.
    #[cfg(feature = "test_utils")]
    pub disable_bootstrap: bool,

    /// If true, will replace the default publish module with a no-op module.
    ///
    /// This flag is only used when [HolochainP2pConfig::k2_test_builder] is true.
    #[cfg(feature = "test_utils")]
    pub disable_publish: bool,

    /// If true, will leave the default no-op gossip module in place rather than replacing it with
    /// the real gossip module.
    ///
    /// This flag is only used when [HolochainP2pConfig::k2_test_builder] is true.
    #[cfg(feature = "test_utils")]
    pub disable_gossip: bool,

    /// Request using the in-memory bootstrap module instead of the real one.
    #[cfg(feature = "test_utils")]
    pub mem_bootstrap: bool,
}

impl std::fmt::Debug for HolochainP2pConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("HolochainP2pConfig");
        dbg.field("compat", &self.compat);
        dbg.field("auth_material", &self.auth_material);

        #[cfg(feature = "test_utils")]
        {
            dbg.field("k2_test_builder", &self.k2_test_builder)
                .field("disable_bootstrap", &self.disable_bootstrap)
                .field("disable_publish", &self.disable_publish)
                .field("disable_gossip", &self.disable_gossip);
        }

        dbg.finish()
    }
}

impl Default for HolochainP2pConfig {
    fn default() -> Self {
        Self {
            get_db_peer_meta: Arc::new(|_| unimplemented!()),
            get_db_op_store: Arc::new(|_| unimplemented!()),
            target_arc_factor: 1,
            auth_material: None,
            network_config: None,
            compat: Default::default(),
            #[cfg(feature = "test_utils")]
            k2_test_builder: false,
            #[cfg(feature = "test_utils")]
            disable_bootstrap: false,
            #[cfg(feature = "test_utils")]
            disable_publish: false,
            #[cfg(feature = "test_utils")]
            disable_gossip: false,
            #[cfg(feature = "test_utils")]
            mem_bootstrap: true,
        }
    }
}

/// See [NetworkCompatParams::proto_ver].
pub const HCP2P_PROTO_VER: u32 = 2;

/// Some parameters used as part of a protocol compatibility check during tx5 preflight
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct NetworkCompatParams {
    /// The current protocol version. This should be incremented whenever
    /// any breaking protocol changes are made to prevent incompatible
    /// nodes from talking to each other.
    pub proto_ver: u32,
}

impl std::fmt::Debug for NetworkCompatParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkCompatParams")
            .field("proto_ver", &self.proto_ver)
            .finish()
    }
}

impl Default for NetworkCompatParams {
    fn default() -> Self {
        Self {
            proto_ver: HCP2P_PROTO_VER,
        }
    }
}
