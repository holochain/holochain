use serde::Deserialize;
use serde::Serialize;

/// Define how Holochain conductor will connect to a keystore, and how
/// to collect the passphrase needed to unlock the keystore.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeystoreConfig {
    /// Enabling this will use a test keystore instead of lair.
    /// This generates publicly accessible private keys.
    /// DO NOT USE THIS IN PRODUCTION!
    DangerTestKeystore,

    /// Connect to an external lair-keystore process.
    /// This keystore type requires a secure passphrase specified
    /// to the cli binary entrypoint for this Holochain conductor process.
    LairServer {
        /// The "connectionUrl" as defined in your "lair-keystore-config.yaml".
        /// This value is also accessible by running `lair-keystore url`.
        connection_url: url2::Url2,
    },

    /// Run a lair-keystore server in-process. It will require exclusive
    /// access to the root directory (no other conductors can share this lair).
    /// This keystore type requires a secure passphrase specified
    /// to the cli binary entrypoint for this Holochain conductor process.
    LairServerInProc {
        /// The "lair_root" path, i.e. the directory containing the
        /// "lair-keystore-config.yaml" file.
        /// If not specified, will default to the ConductorConfig
        /// `[environment_path]/keystore`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lair_root: Option<std::path::PathBuf>,
    },
}

impl Default for KeystoreConfig {
    fn default() -> KeystoreConfig {
        KeystoreConfig::LairServerInProc { lair_root: None }
    }
}
