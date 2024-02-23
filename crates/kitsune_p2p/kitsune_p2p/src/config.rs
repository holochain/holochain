use kitsune_p2p_types::config::{tuning_params_struct, KitsuneP2pTuningParams};
use kitsune_p2p_types::tx2::tx2_adapter::AdapterFactory;
use kitsune_p2p_types::tx_utils::*;
use kitsune_p2p_types::*;
use url2::Url2;

// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default production bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEFAULT: &str = "https://bootstrap-staging.holo.host";

// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default development bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEV: &str = "https://bootstrap-dev.holohost.workers.dev";

/// Configure the kitsune actor.
#[non_exhaustive]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct KitsuneP2pConfig {
    /// List of sub-transports to be included in this pool
    pub transport_pool: Vec<TransportConfig>,

    /// The service used for peers to discover each before they are peers.
    pub bootstrap_service: Option<Url2>,

    /// Network tuning parameters. These are managed loosely,
    /// as they are subject to change. If you specify a tuning parameter
    /// that no longer exists, or a value that does not parse,
    /// a warning will be printed in the tracing log.
    #[serde(default)]
    pub tuning_params: KitsuneP2pTuningParams,

    /// The network used for connecting to other peers
    pub network_type: NetworkType,
}

impl Default for KitsuneP2pConfig {
    fn default() -> Self {
        Self {
            transport_pool: Vec::new(),
            bootstrap_service: None,
            tuning_params: KitsuneP2pTuningParams::default(),
            network_type: NetworkType::QuicBootstrap,
        }
    }
}

impl KitsuneP2pConfig {
    /// This config is making use of tx5 transport
    #[allow(dead_code)] // because of feature flipping
    pub fn is_tx5(&self) -> bool {
        {
            if let Some(t) = self.transport_pool.get(0) {
                return matches!(t, TransportConfig::WebRTC { .. });
            }
        }
        false
    }

    /// Return a copy with the tuning params altered
    pub fn tune(
        mut self,
        f: impl Fn(
            tuning_params_struct::KitsuneP2pTuningParams,
        ) -> tuning_params_struct::KitsuneP2pTuningParams,
    ) -> Self {
        let tp = (*self.tuning_params).clone();
        self.tuning_params = std::sync::Arc::new(f(tp));
        self
    }
}

/// Configure the network bindings for underlying kitsune transports.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportConfig {
    /// A transport that uses the local memory transport protocol
    /// (this is mainly for testing)
    Mem {},

    /// Configure to use Tx5 WebRTC for kitsune networking.
    #[serde(rename = "webrtc", alias = "web_r_t_c", alias = "web_rtc")]
    WebRTC {
        /// The url of the signal server to connect to for addressability.
        signal_url: String,
    },
}
