use std::sync::Arc;

use holochain_conductor_api::{conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver};
use kitsune_p2p::KitsuneP2pConfig;

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

impl SweetConductorConfig {
    /// Standard config for SweetConductors
    pub fn standard() -> Self {
        let mut tuning =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning.gossip_strategy = "sharded-gossip".to_string();

        let mut network = KitsuneP2pConfig::default();
        network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
            bind_to: None,
            override_host: None,
            override_port: None,
        }];
        network.tuning_params = Arc::new(tuning);
        Self(ConductorConfig {
            network: Some(network),
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        })
    }

    /// Completely disable networking
    pub fn no_networking(mut self) -> Self {
        if let Some(c) = self.network.as_mut() {
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
        if let Some(c) = self.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp
            });
        }
        self
    }

    /// Disable publishing and recent gossip
    pub fn historical_only(mut self) -> Self {
        if let Some(c) = self.network.as_mut() {
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
        if let Some(c) = self.network.as_mut() {
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
        if let Some(c) = self.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_historical_gossip = true;
                tp
            });
        }
        self
    }
}
