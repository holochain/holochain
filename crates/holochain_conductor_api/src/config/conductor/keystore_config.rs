use crate::conductor::paths::KeystorePath;
use serde::Deserialize;
use serde::Serialize;

/// Define how secure you want the lair password hashing to be.
/// Note this makes a significant difference to the lair startup time,
/// but also makes a huge difference to how much power is required
/// to crack you passphrases. The default is "Moderate".
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PwHashStrength {
    /// The most intensive password hashing. Note, some devices may
    /// not have enough memory to even run this strong of password hashing.
    Sensitive,

    /// The default best option for password hashing.
    Moderate,

    /// A more light-weight password hashing option, perhaps appropriate
    /// for low power mobile devices. Note, however, that an attacker may
    /// transfer your secrets to a much higher power machine to crack them.
    Interactive,
}

impl Default for PwHashStrength {
    fn default() -> Self {
        Self::Moderate
    }
}

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
        /// `[environment_path]/ks`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lair_root: Option<KeystorePath>,

        /// Password hashing strength to use if we are creating a new
        /// lair keystore. Note, this option will have no effect on
        /// an existing keystore. You cannot change the password hashing
        /// strength after the fact, the database would have to be migrated.
        /// If not specified, "Moderate" will be used.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pw_hash_strength: Option<PwHashStrength>,
    },
}

impl Default for KeystoreConfig {
    fn default() -> KeystoreConfig {
        KeystoreConfig::LairServerInProc {
            lair_root: None,
            pw_hash_strength: None,
        }
    }
}
