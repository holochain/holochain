// Legacy config that will probably change
#![allow(missing_docs)]

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Configure which app instance ID to treat as the DPKI application handler
/// as well as what parameters to pass it on its initialization.
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub struct DpkiConfig {
    /// Path to a DNA which implements the DPKI service, i.e. Deepkey.
    /// Defaults to the built-in Deepkey DNA from the holochain_deepkey_dna crate.
    pub dna_path: Option<PathBuf>,

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
    pub fn new(dna_path: Option<PathBuf>) -> Self {
        Self {
            dna_path,
            allow_throwaway_random_dpki_agent_key: false,
            no_dpki: false,
        }
    }

    pub fn test() -> Self {
        Self {
            dna_path: None,
            allow_throwaway_random_dpki_agent_key: true,
            no_dpki: false,
        }
    }

    pub fn disabled() -> Self {
        Self {
            dna_path: None,
            allow_throwaway_random_dpki_agent_key: false,
            no_dpki: true,
        }
    }
}

impl Default for DpkiConfig {
    fn default() -> Self {
        DpkiConfig::new(None)
    }
}
