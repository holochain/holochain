use super::*;
use crate::actor::*;
use std::time::Duration;

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

/// Callback function to retrieve a conductor database.
pub type GetDbConductor =
    Arc<dyn Fn() -> BoxFut<'static, DbWrite<DbKindConductor>> + 'static + Send + Sync>;

/// Configure reporting.
#[derive(Default)]
pub enum ReportConfig {
    /// No reporting.
    #[default]
    None,

    /// Write reports to a rotating on-disk JsonL file.
    JsonLines(hc_report::HcReportConfig),
}

/// HolochainP2p config struct.
pub struct HolochainP2pConfig {
    /// Callback function to retrieve a peer meta database handle for a dna hash.
    pub get_db_peer_meta: GetDbPeerMeta,

    /// Interval for a pruning task to remove expired values from the peer meta store.
    ///
    /// Default: 10 s
    pub peer_meta_pruning_interval_ms: u64,

    /// Callback function to retrieve an op store database handle for a dna hash.
    pub get_db_op_store: GetDbOpStore,

    /// Callback function to retrieve the conductor database handle.
    pub get_conductor_db: GetDbConductor,

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

    /// The amount of time to elapse before a request times out.
    ///
    /// Defaults to 60 seconds.
    pub request_timeout: Duration,

    /// Configure reporting.
    ///
    /// If `None`, will not report.
    pub report: ReportConfig,

    /// If true, will disable the default bootstrap module.
    ///
    /// This flag is only used in tests.
    #[cfg(feature = "test_utils")]
    pub disable_bootstrap: bool,

    /// If true, will replace the default publish module with a no-op module.
    ///
    /// This flag is only used in tests.
    #[cfg(feature = "test_utils")]
    pub disable_publish: bool,

    /// If true, will leave the default no-op gossip module in place rather than replacing it with
    /// the real gossip module.
    ///
    /// This flag is only used in tests.
    #[cfg(feature = "test_utils")]
    pub disable_gossip: bool,
}

impl std::fmt::Debug for HolochainP2pConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("HolochainP2pConfig");
        dbg.field("compat", &self.compat);
        dbg.field("auth_material", &self.auth_material);
        dbg.field("request_timeout", &self.request_timeout);
        dbg.field("target_arc_factor", &self.target_arc_factor);
        dbg.field("network_config", &self.network_config);

        #[cfg(feature = "test_utils")]
        {
            dbg.field("disable_bootstrap", &self.disable_bootstrap)
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
            peer_meta_pruning_interval_ms: 10_000,
            get_db_op_store: Arc::new(|_| unimplemented!()),
            get_conductor_db: Arc::new(|| unimplemented!()),
            target_arc_factor: 1,
            auth_material: None,
            network_config: None,
            compat: Default::default(),
            request_timeout: Duration::from_secs(60),
            report: ReportConfig::default(),
            #[cfg(feature = "test_utils")]
            disable_bootstrap: false,
            #[cfg(feature = "test_utils")]
            disable_publish: false,
            #[cfg(feature = "test_utils")]
            disable_gossip: false,
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
