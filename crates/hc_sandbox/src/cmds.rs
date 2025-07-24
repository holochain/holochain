use holochain_conductor_api::conductor::NetworkConfig;
use std::path::PathBuf;

use crate::save::HcFile;
use clap::Parser;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use url2::Url2;

// This creates a new Holochain sandbox
// which is a
// - conductor config
// - collection of databases
// - keystore
#[derive(Debug, Parser, Clone)]
pub struct Create {
    /// Number of conductor sandboxes to create.
    #[arg(short, long, default_value = "1")]
    pub num_sandboxes: usize,

    /// Add an optional network config.
    #[command(subcommand)]
    pub network: Option<NetworkCmd>,

    /// Set a root directory for conductor sandboxes to be placed into.
    /// Defaults to the system's temp directory.
    /// This directory must already exist.
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Specify the directory name for each sandbox that is created.
    /// By default, new sandbox directories get a random name
    /// like "kAOXQlilEtJKlTM_W403b".
    /// Use this option to override those names with something explicit.
    /// For example `hc sandbox generate -r path/to/my/chains -n 3 -d=first,second,third`
    /// will create three sandboxes with directories named "first", "second", and "third".
    #[arg(short, long, value_delimiter = ',')]
    pub directories: Vec<PathBuf>,

    /// Launch Holochain with an embedded lair server instead of a standalone process.
    /// Use this option to run the sandboxed conductors when you don't have access to the lair binary.
    #[arg(long)]
    pub in_process_lair: bool,

    /// Set the conductor config CHC (Chain Head Coordinator) URL
    #[cfg(feature = "chc")]
    #[arg(long, value_parser=try_parse_url2)]
    pub chc_url: Option<Url2>,
}

#[derive(Debug, Parser, Clone)]
pub enum NetworkCmd {
    Network(Network),
}

impl NetworkCmd {
    pub fn as_inner(this: &Option<Self>) -> Option<&Network> {
        match this {
            None => None,
            Some(NetworkCmd::Network(n)) => Some(n),
        }
    }
}

#[derive(Debug, Parser, Clone)]
pub struct Network {
    /// Set the type of network.
    #[command(subcommand)]
    pub transport: NetworkType,

    /// Optionally set a bootstrap service URL.
    /// A bootstrap service can used for peers to discover each other without
    /// prior knowledge of each other.
    #[arg(short, long, value_parser = try_parse_url2)]
    pub bootstrap: Option<Url2>,
}

#[derive(Debug, Parser, Clone)]
pub enum NetworkType {
    /// A transport that uses the local memory transport protocol.
    Mem,
    // /// A transport that uses the QUIC protocol.
    // Quic(Quic),
    // /// A transport that uses the MDNS protocol.
    // Mdns,
    /// A transport that uses the WebRTC protocol.
    #[command(name = "webrtc")]
    WebRTC {
        /// URL to a holochain tx5 WebRTC signal server.
        signal_url: String,

        /// Optional path to override webrtc peer connection config file.
        webrtc_config: Option<std::path::PathBuf>,
    },
}

#[derive(Debug, Parser, Clone)]
pub struct Existing {
    /// Run all the existing conductor sandboxes specified in `$(pwd)/.hc`.
    #[arg(short, long, conflicts_with_all = &["last", "indices"])]
    pub all: bool,
    /// Run a selection of existing conductor sandboxes
    /// from those specified in `$(pwd)/.hc`.
    /// Existing sandboxes and their indices are visible via `hc list`.
    /// Use the zero-based index to choose which sandboxes to use.
    /// For example `hc sandbox run 1 3 5` or `hc sandbox run 1`
    #[arg(conflicts_with_all = &["all", "last"])]
    pub indices: Vec<usize>,
}

impl Existing {

    pub fn none() -> Self { Existing { all: false, indices: vec![] } }
    pub fn all() -> Self { Existing { all: true, indices: vec![] } }
    pub fn one(index: usize) -> Self { Existing { all: false, indices: vec![index] } }
    pub fn many(indices: Vec<usize>) -> Self { Existing { all: false, indices } }

    /// Determine all sandbox paths to use from an `.hc` file based on this struct's state.
    pub fn load(&self, hc_file: &HcFile) -> std::io::Result<Vec<ConfigRootPath>> {
        if self.all {
            // Warn for all invalid paths
            hc_file
                .invalid_paths()
                .iter()
                .enumerate()
                .for_each(|(i, inv)| msg!("Warning. Sandbox not found at {}: {}", i, inv.display()));
            // Return all valid sandboxes in .hc
            return Ok(hc_file.valid_paths());
        }
        if !self.indices.is_empty() {
            let mut selection = Vec::new();
            // Get the indices
            for i in self.indices.clone() {
                let Some(Ok(selected)) = hc_file.existing_all.get(i) else {
                    return Err(std::io::Error::other(format!(
                        "Aborting. No sandbox found at index {}.",
                        i
                    )));
                };
                selection.push(selected.clone());
            }
            return Ok(selection);
        }
        // No options provided, pick one known sandbox
        match hc_file.valid_paths().len() {
            1 => Ok(vec![hc_file.valid_paths()[0].clone()]), // If there is only one saved sandbox then use that.
            0 => {
                // There are no sandboxes
                msg!(
                    "
Before running or calling you need to generate a sandbox.
You can use `hc sandbox generate` to do this.
Run `hc sandbox generate --help` for more options."
                );
                Ok(vec![])
            }
            _ => {
                // There are multiple saved sandboxes, the user must disambiguate
                msg!(
                    "
There are multiple sandboxes and hc doesn't know which of them to run.
You can run:
    - `--all` `-a` run all sandboxes.
    - `1` run a sandbox by index from the list below.
    - `0 2` run multiple sandboxes by indices from the list below.
Run `hc sandbox list` to see the sandboxes or `hc sandbox run --help` for more information."
                );
                hc_file.list(false)?;
                Ok(vec![])
            }
        }
    }

    /// Returns true if no "existing" option has been set
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty() && !self.all
    }
}

impl Network {
    pub async fn to_kitsune(this: &Option<&Self>) -> Option<NetworkConfig> {
        let Network {
            transport,
            bootstrap,
        } = match this {
            None => {
                return Some(NetworkConfig {
                    advanced: Some(serde_json::json!({
                        // Allow plaintext signal for hc sandbox to have it work with local
                        // signaling servers spawned by kitsune2-bootstrap-srv
                        "tx5Transport": {
                            "signalAllowPlainText": true,
                        }
                    })),
                    ..NetworkConfig::default()
                });
            }
            Some(n) => (*n).clone(),
        };

        let mut kit = NetworkConfig::default();
        if let Some(bootstrap) = bootstrap {
            kit.bootstrap_url = bootstrap;
        }

        match transport {
            NetworkType::Mem => (),
            NetworkType::WebRTC {
                signal_url,
                webrtc_config,
            } => {
                let webrtc_config = match webrtc_config {
                    Some(path) => {
                        let content = tokio::fs::read_to_string(path)
                            .await
                            .expect("failed to read webrtc_config file");
                        let parsed = serde_json::from_str(&content)
                            .expect("failed to parse webrtc_config file content");
                        Some(parsed)
                    }
                    None => None,
                };
                kit.signal_url = url2::url2!("{}", signal_url);
                kit.webrtc_config = webrtc_config;
                kit.advanced = Some(serde_json::json!({
                    // Allow plaintext signal for hc sandbox to have it work with local
                    // signaling servers spawned by kitsune2-bootstrap-srv
                    "tx5Transport": {
                        "signalAllowPlainText": true,
                    }
                }));
            }
        }
        Some(kit)
    }
}

impl Default for Create {
    fn default() -> Self {
        Self {
            num_sandboxes: 1,
            network: None,
            root: None,
            directories: Vec::with_capacity(0),
            in_process_lair: false,
            #[cfg(feature = "chc")]
            chc_url: None,
        }
    }
}

// The only purpose for this wrapper function is to get around a type inference failure.
// Plenty of search hits out there for "implementation of `FnOnce` is not general enough"
// e.g., https://users.rust-lang.org/t/implementation-of-fnonce-is-not-general-enough/68294
fn try_parse_url2(arg: &str) -> url2::Url2Result<Url2> {
    Url2::try_parse(arg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_is_empty() {
        assert!(Existing::none().is_empty());
        assert!(!Existing::all().is_empty());
        assert!(!Existing::one(1).is_empty());
        assert!(!Existing::many(vec![1, 2, 3]).is_empty());
    }

    #[test]
    fn existing_load_normal() {
        let hc_file = crate::save::tests::create_test_folder(vec![
            Ok(".ok1"),
            Err(".err1"),
            Ok(".ok2"),
            Ok(".ok3"),
            Err(".err2"),
        ]);

        let res = Existing::none().load(&hc_file).unwrap();
        assert_eq!(0, res.len());

        let res = Existing::all().load(&hc_file).unwrap();
        assert_eq!(3, res.len());

        let res = Existing::one(0).load(&hc_file).unwrap();
        assert_eq!(1, res.len());

        let res = Existing::one(1).load(&hc_file);
        assert!(res.is_err());

        let res = Existing::many(vec![1, 2, 3]).load(&hc_file);
        assert!(res.is_err());

        let res = Existing::many(vec![0, 3]).load(&hc_file).unwrap();
        assert_eq!(2, res.len());

        let res = Existing::many(vec![0, 42]).load(&hc_file);
        assert!(res.is_err());
    }

    #[test]
    fn existing_load_empty() {
        let hc_file = crate::save::tests::create_test_folder(Vec::new());

        let res = Existing::none().load(&hc_file).unwrap();
        assert_eq!(0, res.len());

        let res = Existing::all().load(&hc_file).unwrap();
        assert_eq!(0, res.len());

        let res =  Existing::one(0).load(&hc_file);
        assert!(res.is_err());

        let res = Existing::many(vec![1, 2, 3]).load(&hc_file);
        assert!(res.is_err());
    }
}
