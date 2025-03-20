use std::sync::atomic::AtomicUsize;

use holochain_conductor_api::{
    conductor::{ConductorConfig, ConductorTuningParams, NetworkConfig},
    AdminInterfaceConfig, InterfaceDriver,
};
use holochain_types::websocket::AllowedOrigins;

use super::{DynSweetRendezvous, SweetConductor};

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

        self
    }

    /// Standard config for SweetConductors
    pub fn standard() -> Self {
        let network_config = NetworkConfig::default()
            .with_gossip_initiate_interval_ms(1000)
            .with_gossip_min_initiate_interval_ms(1000)
            .with_gossip_round_timeout_ms(10_000);

        let mut c = SweetConductorConfig::from(network_config).tune_conductor(|tune| {
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
            config.network.bootstrap_url = url2::url2!("rendezvous:");
        }

        config.network.signal_url = url2::url2!("rendezvous:");

        config
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
