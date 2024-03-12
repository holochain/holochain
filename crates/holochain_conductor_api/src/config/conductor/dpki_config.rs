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
}
