use std::sync::Arc;

use crate::sweettest::SweetRendezvous;
use holochain_conductor_api::{conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver};
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::nanoid::nanoid;

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
            network: Some(network),
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        }
        .into()
    }
}

impl SweetConductorConfig {
    /// Convert into a ConductorConfig.
    pub async fn into_conductor_config(self, rendezvous: &dyn SweetRendezvous) -> ConductorConfig {
        let mut config = self.0;

        if let Some(n) = config.network.as_mut() {
            if n.bootstrap_service.is_some()
                && n.bootstrap_service.as_ref().unwrap().to_string() == "rendezvous:"
            {
                n.bootstrap_service = Some(url2::url2!("{}", rendezvous.bootstrap_addr()));
            }

            #[cfg(feature = "tx5")]
            {
                for t in n.transport_pool.iter_mut() {
                    if let kitsune_p2p::TransportConfig::WebRTC { signal_url } = t {
                        if signal_url == "rendezvous:" {
                            *signal_url = rendezvous.sig_addr().to_string();
                        }
                    }
                }
            }
        }

        tracing::info!(?config);

        config
    }

    /// Standard config for SweetConductors
    pub fn standard() -> Self {
        let mut config: Self = KitsuneP2pConfig::default().into();
        config.random_scope();
        config
    }

    /// Rendezvous config for SweetConductors
    pub fn rendezvous() -> Self {
        let mut tuning =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning.gossip_strategy = "sharded-gossip".to_string();

        let mut network = KitsuneP2pConfig::default();
        network.bootstrap_service = Some(url2::url2!("rendezvous:"));

        /*#[cfg(not(feature = "tx5"))]
        {
            network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
                bind_to: None,
                override_host: None,
                override_port: None,
            }];
        }*/

        #[cfg(feature = "tx5")]
        {
            network.transport_pool = vec![kitsune_p2p::TransportConfig::WebRTC {
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
        let r = &mut self
            .0
            .network
            .as_mut()
            .expect("failed to tune network")
            .tuning_params;
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
        self.0
            .network
            .as_mut()
            .expect("failed to tune network")
            .tuning_params = Arc::new(tuning_params);
        self
    }

    /// Set the tracing scope to a new random value
    pub fn random_scope(&mut self) {
        let scope = nanoid!();
        let network = self.network.get_or_insert_with(Default::default);
        network.tracing_scope = Some(scope.clone());
        self.tracing_scope = Some(scope);
    }

    /// Completely disable networking
    pub fn no_networking(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_recent_gossip = true;
                tp.disable_historical_gossip = true;
                tp
            });
        }
        self
    }

    /// Disable publishing
    pub fn no_publish(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp
            });
        }
        self
    }

    /// Disable publishing and recent gossip
    pub fn historical_only(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_recent_gossip = true;
                tp
            });
        }
        self
    }

    /// Disable recent op gossip, but keep agent gossip
    pub fn historical_and_agent_gossip_only(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                // keep recent gossip for agent gossip, but gossip no ops.
                tp.danger_gossip_recent_threshold_secs = 0;
                tp
            });
        }
        self
    }

    /// Disable publishing and historical gossip
    pub fn recent_only(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_historical_gossip = true;
                tp
            });
        }
        self
    }
}
