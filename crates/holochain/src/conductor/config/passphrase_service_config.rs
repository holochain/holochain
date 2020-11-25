use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

/// The default passphrase service is `Cmd` which will ask for a passphrase via stdout stdin.
/// In the context of a UI that wraps the conductor, this way of providing passphrases
/// is not feasible.
/// Setting the type to "unixsocket" and providing a path to a file socket enables
/// arbitrary UIs to connect to the conductor and prompt the user for a passphrase.
/// The according `PassphraseServiceUnixSocket` will send a request message over the socket
/// then receives bytes as passphrase until a newline is sent.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PassphraseServiceConfig {
    /// Passphrase is requested from the command line
    Cmd,
    /// Passphrase is requested over a Unix domain socket at the given path.
    UnixSocket {
        /// Path of the socket
        path: PathBuf,
    },
    /// Passphrase is pulled directly from the config file
    FromConfig {
        /// The actual pasphrase
        passphrase: String,
    },
}

impl Default for PassphraseServiceConfig {
    fn default() -> PassphraseServiceConfig {
        PassphraseServiceConfig::Cmd
    }
}
