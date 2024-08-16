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

    /// The lair tag used to refer to the "device seed" which was used to generate
    /// the AgentPubKey for the DPKI cell
    pub device_seed_lair_tag: String,

    /// For testing only, we can turn off DPKI if needed.
    /// TODO: this can be removed once DPKI is truly optional again.
    #[serde(default)]
    pub no_dpki: bool,
}

impl DpkiConfig {
    pub fn new(dna_path: Option<PathBuf>, device_seed_lair_tag: String) -> Self {
        Self {
            dna_path,
            device_seed_lair_tag,
            no_dpki: false,
        }
    }

    pub fn disabled() -> Self {
        Self {
            dna_path: None,
            device_seed_lair_tag: "disabled".to_string(),
            no_dpki: true,
        }
    }
}

impl Default for DpkiConfig {
    fn default() -> Self {
        DpkiConfig::new(None, "DPKI_DEVICE_SEED".to_string())
    }
}
