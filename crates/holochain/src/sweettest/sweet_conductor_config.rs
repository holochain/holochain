use super::{DynSweetRendezvous, SweetConductor};
use holochain_conductor_api::{
    conductor::{ConductorConfig, ConductorTuningParams, NetworkConfig},
    AdminInterfaceConfig, InterfaceDriver,
};
use holochain_types::websocket::AllowedOrigins;
use std::sync::atomic::AtomicUsize;

pub(crate) static NUM_CREATED: AtomicUsize = AtomicUsize::new(0);

/// Wrapper around ConductorConfig with some helpful builder methods
#[derive(Clone, derive_more::Deref, derive_more::DerefMut, derive_more::Into)]
pub struct SweetConductorConfig {
    #[deref]
    #[deref_mut]
    #[into]
    config: ConductorConfig,

    // Helps to keep owned references alive
    rendezvous: Option<DynSweetRendezvous>,
}

impl From<ConductorConfig> for SweetConductorConfig {
    fn from(config: ConductorConfig) -> Self {
        Self {
            config,
            rendezvous: None,
        }
    }
}

impl From<NetworkConfig> for SweetConductorConfig {
    fn from(network: NetworkConfig) -> Self {
        ConductorConfig {
            network,
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port: 0,
                    danger_bind_addr: None,
                    allowed_origins: AllowedOrigins::Any,
                },
            }]),
            tuning_params: Some(ConductorTuningParams {
                sys_validation_retry_delay: Some(std::time::Duration::from_secs(1)),
                countersigning_resolution_retry_delay: Some(std::time::Duration::from_secs(3)),
                countersigning_resolution_retry_limit: None,
                publish_trigger_interval: None,
                min_publish_interval: None,
                disable_self_validation: false,
                disable_warrant_issuance: false,
            }),
            ..Default::default()
        }
        .into()
    }
}

impl SweetConductorConfig {
    /// Standard config for SweetConductors, disabling bootstrapping.
    pub fn standard() -> Self {
        let mut network_config = NetworkConfig::default()
            .with_gossip_initiate_interval_ms(1000)
            .with_gossip_initiate_jitter_ms(100)
            .with_gossip_min_initiate_interval_ms(1000)
            .with_gossip_round_timeout_ms(10_000);

        network_config.bootstrap_url = url2::url2!("rendezvous:");
        network_config.signal_url = url2::url2!("rendezvous:");
        network_config.relay_url = url2::url2!("rendezvous:");

        SweetConductorConfig::from(network_config).tune_conductor(|tune| {
            tune.sys_validation_retry_delay = Some(std::time::Duration::from_secs(1));
        })
    }

    /// Rendezvous config for SweetConductors
    pub fn rendezvous(bootstrap: bool) -> Self {
        let mut config = Self::standard();

        if !bootstrap {
            config.network.disable_bootstrap = true;
        }

        config
    }
    /// Rewrite the config to point to the given rendezvous server
    pub fn apply_rendezvous(mut self, rendezvous: &DynSweetRendezvous) -> Self {
        self.rendezvous = Some(rendezvous.clone());
        let network = &mut self.network;

        if network.bootstrap_url.as_str() == "rendezvous:" {
            network.bootstrap_url = url2::url2!("{}", rendezvous.bootstrap_addr());
        }

        if network.signal_url.as_str() == "rendezvous:" {
            network.signal_url = url2::url2!("{}", rendezvous.sig_addr());
        }

        if network.relay_url.as_str() == "rendezvous:" {
            network.relay_url = url2::url2!("{}", rendezvous.sig_addr());
        }

        self
    }

    /// Getter
    pub fn get_rendezvous(&self) -> Option<DynSweetRendezvous> {
        self.rendezvous.clone()
    }

    /// Build a SweetConductor from this config
    pub async fn build_conductor(self) -> SweetConductor {
        SweetConductor::from_config(self).await
    }

    /// Apply a function to the conductor's tuning parameters to customise them.
    pub fn tune_conductor(mut self, f: impl FnOnce(&mut ConductorTuningParams)) -> Self {
        if let Some(ref mut params) = self.tuning_params {
            f(params);
        }
        self
    }

    /// Apply a function to the network config to customise it.
    pub fn tune_network_config(mut self, f: impl FnOnce(&mut NetworkConfig)) -> Self {
        f(&mut self.network);
        self
    }
}
