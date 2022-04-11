use serde::Deserialize;
use serde::Serialize;

/// Define how Holochain conductor will connect to a keystore, and how
/// to collect the passphrase needed to unlock the keystore.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum KeystoreConfig {
    /// Enabling this will use a test keystore instead of lair.
    /// This generates publicly accessible private keys.
    /// DO NOT USE THIS IN PRODUCTION!
    /// (this uses the legacy lair keystore api)
    DangerTestKeystoreLegacyDeprecated,

    /// Connect to an external lair-keystore process.
    /// (this uses the legacy lair keystore api)
    LairServerLegacyDeprecated {
        /// Optional path for keystore directory. If not specified,
        /// will use the default provided by:
        /// [ConfigBuilder](https://docs.rs/lair_keystore_api/0.0.1-alpha.4/lair_keystore_api/struct.ConfigBuilder.html)
        #[serde(default)]
        keystore_path: Option<std::path::PathBuf>,

        /// DANGER - THIS IS NOT SECURE--In fact, it defeats the
        /// whole purpose of having a passphrase in the first place!
        /// Passphrase is pulled directly from the config file.
        danger_passphrase_insecure_from_config: String,
    },

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
    //
    // DISABLED - we can't pull the full lair_keystore crate in as a dep
    //            until we make db-encryption feature the default.
    // /// Run a lair-keystore server in-process. It will require exclusive
    // /// access to the root directory (no other conductors can share this lair).
    // /// This keystore type requires a secure passphrase specified
    // /// to the cli binary entrypoint for this Holochain conductor process.
    // LairServerInProc {
    //     /// The "lair_root" path, i.e. the directory containing the
    //     /// "lair-keystore-config.yaml" file.
    //     lair_root: std::path::PathBuf,
    // },
}

impl Default for KeystoreConfig {
    fn default() -> KeystoreConfig {
        // Not a great default, but it's all we have to work with
        // until we get the full new lair api in place,
        // at which point we should switch to LairServerInProc,
        // with an auto-generated lair-config if it doesn't exist.
        KeystoreConfig::LairServerLegacyDeprecated {
            keystore_path: None,
            danger_passphrase_insecure_from_config: "default-insecure-passphrase".into(),
        }
    }
}
