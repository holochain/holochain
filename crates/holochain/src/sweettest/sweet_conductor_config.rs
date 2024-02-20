use std::sync::{atomic::AtomicUsize, Arc};

use crate::sweettest::SweetRendezvous;
use holochain_conductor_api::{
    conductor::{ConductorConfig, ConductorTuningParams},
    AdminInterfaceConfig, InterfaceDriver,
};
use kitsune_p2p_types::config::KitsuneP2pConfig;

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
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            tuning_params: Some(ConductorTuningParams {
                sys_validation_retry_delay: Some(std::time::Duration::from_secs(1)),
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
                if let kitsune_p2p_types::config::TransportConfig::WebRTC { signal_url } = t {
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
        KitsuneP2pConfig::default().into()
    }

    /// Rendezvous config for SweetConductors
    pub fn rendezvous(bootstrap: bool) -> Self {
        let mut tuning =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning.gossip_strategy = "sharded-gossip".to_string();

        let mut network = KitsuneP2pConfig::default();
        if bootstrap {
            network.bootstrap_service = Some(url2::url2!("rendezvous:"));
        }

        /*#[cfg(not(feature = "tx5"))]
        {
            network.transport_pool = vec![TransportConfig::Quic {
                bind_to: None,
                override_host: None,
                override_port: None,
            }];
        }*/

        #[cfg(feature = "tx5")]
        {
            network.transport_pool = vec![kitsune_p2p_types::config::TransportConfig::WebRTC {
                signal_url: "rendezvous:".into(),
            }];
        }

        network.tuning_params = Arc::new(tuning);
        network.into()
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
