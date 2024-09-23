use std::sync::{atomic::AtomicUsize, Arc};

use crate::sweettest::SweetRendezvous;
use holochain_conductor_api::{
    conductor::{ConductorConfig, ConductorTuningParams},
    AdminInterfaceConfig, InterfaceDriver,
};
use holochain_types::websocket::AllowedOrigins;
use kitsune_p2p_types::config::KitsuneP2pConfig;

use super::SweetConductor;

pub(crate) static NUM_CREATED: AtomicUsize = AtomicUsize::new(0);

/// Wrapper around ConductorConfig with some helpful builder methods
#[derive(
    Clone,
    Debug,
    PartialEq,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
    derive_more::Into,
)]
pub struct SweetConductorConfig(ConductorConfig);

impl From<KitsuneP2pConfig> for SweetConductorConfig {
    fn from(network: KitsuneP2pConfig) -> Self {
        ConductorConfig {
            network,
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port: 0,
                    allowed_origins: AllowedOrigins::Any,
                },
            }]),
            tuning_params: Some(ConductorTuningParams {
                sys_validation_retry_delay: Some(std::time::Duration::from_secs(1)),
                countersigning_resolution_retry_delay: Some(std::time::Duration::from_secs(3)),
                countersigning_resolution_retry_limit: None,
                min_publish_interval: None,
            }),
            ..Default::default()
        }
        .into()
    }
}

impl SweetConductorConfig {
    /// Convert into a ConductorConfig.
    pub async fn into_conductor_config(self, rendezvous: &dyn SweetRendezvous) -> ConductorConfig {
        let mut config = self.0;
        let network = &mut config.network;

        if network.bootstrap_service.is_some()
            && network.bootstrap_service.as_ref().unwrap().to_string() == "rendezvous:"
        {
            network.bootstrap_service = Some(url2::url2!("{}", rendezvous.bootstrap_addr()));
        }

        #[cfg(feature = "tx5")]
        {
            for t in network.transport_pool.iter_mut() {
                if let kitsune_p2p_types::config::TransportConfig::WebRTC { signal_url, .. } = t {
                    if signal_url == "rendezvous:" {
                        *signal_url = rendezvous.sig_addr().to_string();
                    }
                }
            }
        }

        config
    }

    /// Standard config for SweetConductors
    pub fn standard() -> Self {
        let mut c = SweetConductorConfig::from(KitsuneP2pConfig::default())
            .tune(|tune| {
                tune.gossip_loop_iteration_delay_ms = 500;
                tune.gossip_peer_on_success_next_gossip_delay_ms = 1000;
                tune.gossip_peer_on_error_next_gossip_delay_ms = 1000;
                tune.gossip_round_timeout_ms = 10_000;
            })
            .tune_conductor(|tune| {
                tune.sys_validation_retry_delay = Some(std::time::Duration::from_secs(1));
            });

        // Allow device seed generation to exercise key derivation in sweettests.
        c.device_seed_lair_tag = Some("sweet-conductor-device-seed".to_string());
        c.danger_generate_throwaway_device_seed = true;
        c
    }

    /// Disable DPKI, which is on by default.
    /// You would want to disable DPKI in situations where you're testing unusual situations
    /// such as tests which disable networking, tests which use pregenerated agent keys,
    /// or any situation where it's known that DPKI is irrelevant.
    pub fn no_dpki(mut self) -> Self {
        self.dpki = holochain_conductor_api::conductor::DpkiConfig::disabled();
        self
    }

    /// Disable DPKI in a situation where we would like to run DPKI in a test, but the test
    /// only passes if it's disabled and we can't figure out why.
    #[cfg(feature = "test_utils")]
    pub fn no_dpki_mustfix(mut self) -> Self {
        tracing::warn!("Disabling DPKI for a test which should pass with DPKI enabled. TODO: fix");
        self.dpki = holochain_conductor_api::conductor::DpkiConfig::disabled();
        self
    }

    /// Rendezvous config for SweetConductors
    pub fn rendezvous(bootstrap: bool) -> Self {
        let mut config = Self::standard();

        if bootstrap {
            config.network.bootstrap_service = Some(url2::url2!("rendezvous:"));
        }

        /*#[cfg(not(feature = "tx5"))]
        {
            config.network.transport_pool = vec![TransportConfig::Quic {
                bind_to: None,
                override_host: None,
                override_port: None,
            }];
        }*/

        #[cfg(feature = "tx5")]
        {
            config.network.transport_pool =
                vec![kitsune_p2p_types::config::TransportConfig::WebRTC {
                    signal_url: "rendezvous:".into(),
                    webrtc_config: None,
                }];
        }

        config
    }

    /// Build a SweetConductor from this config
    pub async fn build_conductor(self) -> SweetConductor {
        SweetConductor::from_config(self).await
    }

    /// Set network tuning params.
    pub fn tune(
        mut self,
        f: impl FnOnce(&mut kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams),
    ) -> Self {
        let r = &mut self.0.network.tuning_params;
        let mut tuning = (**r).clone();
        f(&mut tuning);
        *r = Arc::new(tuning);
        self
    }

    /// Set network tuning params.
    pub fn set_tuning_params(
        mut self,
        tuning_params: kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams,
    ) -> Self {
        self.0.network.tuning_params = Arc::new(tuning_params);
        self
    }

    /// Apply a function to the conductor's tuning parameters to customise them.
    pub fn tune_conductor(mut self, f: impl FnOnce(&mut ConductorTuningParams)) -> Self {
        if let Some(ref mut params) = self.0.tuning_params {
            f(params);
        }
        self
    }

    /// Completely disable networking
    pub fn no_networking(mut self) -> Self {
        self.0.network = self.0.network.clone().tune(|mut tp| {
            tp.disable_publish = true;
            tp.disable_recent_gossip = true;
            tp.disable_historical_gossip = true;
            tp
        });
        self
    }

    /// Disable publishing
    pub fn no_publish(mut self) -> Self {
        self.0.network = self.0.network.clone().tune(|mut tp| {
            tp.disable_publish = true;
            tp
        });
        self
    }

    /// Disable publishing and recent gossip
    pub fn historical_only(mut self) -> Self {
        self.0.network = self.0.network.clone().tune(|mut tp| {
            tp.disable_publish = true;
            tp.disable_recent_gossip = true;
            tp
        });
        self
    }

    /// Disable recent op gossip, but keep agent gossip
    pub fn historical_and_agent_gossip_only(mut self) -> Self {
        self.0.network = self.0.network.clone().tune(|mut tp| {
            tp.disable_publish = true;
            // keep recent gossip for agent gossip, but gossip no ops.
            tp.danger_gossip_recent_threshold_secs = 0;
            tp
        });
        self
    }

    /// Disable publishing and historical gossip
    pub fn recent_only(mut self) -> Self {
        self.0.network = self.0.network.clone().tune(|mut tp| {
            tp.disable_publish = true;
            tp.disable_historical_gossip = true;
            tp
        });
        self
    }
}
