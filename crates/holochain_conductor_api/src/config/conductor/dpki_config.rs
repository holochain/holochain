// Legacy config that will probably change
#![allow(missing_docs)]

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// The network seed used in the main "production" DPKI network.
const DPKI_NETWORK_SEED_MAIN: &str = "deepkey-main";

/// A network seed used for testing.
const DPKI_NETWORK_SEED_TESTING: &str = "deepkey-testing";

/// Configure which app instance ID to treat as the DPKI application handler
/// as well as what parameters to pass it on its initialization.
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub struct DpkiConfig {
    /// Path to a DNA which implements the DPKI service, i.e. Deepkey.
    /// Defaults to the built-in Deepkey DNA from the holochain_deepkey_dna crate.
    pub dna_path: Option<PathBuf>,

    /// **IMPORTANT!**
    ///
    /// For the main DPKI network, this seed must be set to "deepkey-main".
    /// For hApp unit and integration tests, a random seed should be used.
    ///
    /// DPKI is always installed with a network seed.
    /// Also, any two conductors not using the exact same DPKI service cannot communicate with each other.
    /// This means that this network seed much match across all conductors in a network!
    //
    // TODO: consider emitting a warning if this is not set to the production value
    //       in release builds.
    pub network_seed: String,

    /// Allow the DPKI agent key to be generated randomly in the absence of a
    /// [`ConductorConfig::device_seed_lair_tag`] setting. This is useful in test
    /// environments where the device seed is not set and key regeneration is not
    /// needed. For any real use of Holochain, do not set this to true!
    #[serde(default)]
    pub allow_throwaway_random_dpki_agent_key: bool,

    /// For testing only, we can turn off DPKI if needed.
    /// TODO: this can be removed once DPKI is truly optional again.
    #[serde(default)]
    pub no_dpki: bool,
}

impl DpkiConfig {
    pub fn production(dna_path: Option<PathBuf>) -> Self {
        Self {
            dna_path,
            network_seed: DPKI_NETWORK_SEED_MAIN.to_string(),
            allow_throwaway_random_dpki_agent_key: false,
            no_dpki: false,
        }
    }

    pub fn testing() -> Self {
        Self {
            dna_path: None,
            network_seed: DPKI_NETWORK_SEED_TESTING.to_string(),
            allow_throwaway_random_dpki_agent_key: true,
            no_dpki: false,
        }
    }

    pub fn disabled() -> Self {
        Self {
            dna_path: None,
            network_seed: "".to_string(),
            allow_throwaway_random_dpki_agent_key: false,
            no_dpki: true,
        }
    }
}

impl Default for DpkiConfig {
    fn default() -> Self {
        Self {
            dna_path: None,
            network_seed: DPKI_NETWORK_SEED_TESTING.to_string(),
            allow_throwaway_random_dpki_agent_key: false,
            no_dpki: false,
        }
    }
}
