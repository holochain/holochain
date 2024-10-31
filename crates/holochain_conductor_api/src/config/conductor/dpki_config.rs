// Legacy config that will probably change
#![allow(missing_docs)]

use std::path::PathBuf;

use cfg_if::cfg_if;
use serde::Deserialize;
use serde::Serialize;

#[cfg(feature = "unstable-dpki")]
/// The network seed used in the main "production" DPKI network.
const DPKI_NETWORK_SEED_MAIN: &str = "deepkey-main";

#[cfg(feature = "unstable-dpki")]
/// A network seed used for testing.
const DPKI_NETWORK_SEED_TESTING: &str = "deepkey-testing";

/// Configure DPKI properties.
///
/// Note that the Deepkey DNA path and the network seed settings determine network compatibility.
/// They have to match for all conductors on a network, for them to be able to communicate.
/// Also see [`holochain_p2p::spawn::NetworkCompatParams`].
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub struct DpkiConfig {
    /// Path to a DNA which implements the DPKI service, i.e. Deepkey.
    /// Defaults to the built-in Deepkey DNA from the holochain_deepkey_dna crate.
    pub dna_path: Option<PathBuf>,

    /// DPKI is always installed with a network seed.
    /// Only conductors using the exact same DPKI service can communicate with each other.
    /// This means that this network seed must match across all conductors in a network.
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
    // TODO: this can be removed once DPKI is truly optional again.
    #[serde(default)]
    pub no_dpki: bool,
}

impl DpkiConfig {
    #[cfg(feature = "unstable-dpki")]
    pub fn production(dna_path: Option<PathBuf>) -> Self {
        Self {
            dna_path,
            network_seed: DPKI_NETWORK_SEED_MAIN.to_string(),
            allow_throwaway_random_dpki_agent_key: false,
            no_dpki: false,
        }
    }

    #[cfg(feature = "unstable-dpki")]
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
        cfg_if! {
            if #[cfg(feature = "unstable-dpki")] {
                Self::testing()
            } else {
                Self::disabled()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DpkiConfig;

    #[cfg(not(feature = "unstable-dpki"))]
    #[test]
    fn default_config() {
        let config = DpkiConfig::default();
        assert_eq!(config, DpkiConfig::disabled());
    }

    #[cfg(feature = "unstable-dpki")]
    #[test]
    fn default_config_with_feature_enabled() {
        let config = DpkiConfig::default();
        assert_eq!(config, DpkiConfig::default());
    }

    #[cfg(feature = "unstable-dpki")]
    #[test]
    fn testing_config_with_feature_enabled() {
        let config = DpkiConfig::default();
        assert_eq!(config, DpkiConfig::default());
    }

    #[cfg(feature = "unstable-dpki")]
    #[test]
    fn production_config_with_feature_enabled() {
        let config = DpkiConfig::production(None);
        assert_eq!(config, DpkiConfig::production(None));
    }
}
