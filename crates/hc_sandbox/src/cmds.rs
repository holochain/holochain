use holochain_conductor_api::conductor::NetworkConfig;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::Parser;
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
    pub num_sandboxes: NonZeroUsize,

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
    #[arg(short, long, conflicts_with = "indices")]
    pub all: bool,

    /// Run a selection of existing conductor sandboxes
    /// from those specified in `$(pwd)/.hc`.
    /// Existing sandboxes and their indices are visible via `hc list`.
    /// Use the zero-based index to choose which sandboxes to use.
    /// For example `hc sandbox run 1 3 5` or `hc sandbox run 1`
    #[arg(conflicts_with = "all")]
    pub indices: Vec<usize>,
}

impl Existing {
    pub fn load(self, hc_dir: PathBuf) -> std::io::Result<Vec<PathBuf>> {
        let sandboxes = crate::save::load(hc_dir)?;
        if sandboxes.is_empty() {
            // There are no sandboxes
            msg!(
                "
Before running or calling you need to generate a sandbox.
You can use `hc sandbox generate` or `hc sandbox create` to do this.
Run `hc sandbox generate --help` or `hc sandbox create --help` for more options."
            );
            Err(std::io::Error::other("No sandboxes found."))
        } else if self.all {
            // Report any missing sandbox
            sandboxes
                .iter()
                .enumerate()
                .filter_map(|(i, result)| result.as_ref().err().map(|path| (i, path)))
                .for_each(|(i, path)| {
                    msg!(
                        "Missing sandbox: {}:{}",
                        i,
                        path.as_path().to_string_lossy()
                    )
                });
            // Return all available sandboxes.
            Ok(sandboxes.into_iter().flatten().collect())
        } else if !self.indices.is_empty() {
            // Return all sandboxes at provided indices.
            // Return an error if any index is out of bounds or if a sandbox is missing at any given index.
            let mut set = std::collections::HashSet::with_capacity(self.indices.len());
            let mut selected = Vec::with_capacity(self.indices.len());
            for i in self.indices.into_iter().filter(|x| set.insert(*x)) {
                let Some(result) = sandboxes.get(i) else {
                    return Err(std::io::Error::other(format!(
                        "Index {} is out of bounds.",
                        i
                    )));
                };
                match result {
                    Err(path) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            format!("Missing sandbox {}:{}", i, path.as_path().to_string_lossy()),
                        ))
                    }
                    Ok(path) => selected.push(path.clone()),
                }
            }
            Ok(selected)
        } else if sandboxes.len() == 1 {
            // If there is only one sandbox, then return that
            match &sandboxes[0] {
                Err(path) => {
                    msg!("Missing sandbox {}:{}", 0, path.as_path().to_string_lossy());
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Missing sandbox {}:{}", 0, path.as_path().to_string_lossy()),
                    ))
                }
                Ok(path) => Ok(vec![path.clone()]),
            }
        } else {
            // There are multiple sandboxes, the user must disambiguate
            msg!(
                "
There are multiple sandboxes and hc doesn't know which one to run.
You can run:
    - `--all` `-a` run all sandboxes.
    - `1` run a sandbox by index from the list below.
    - `0 2` run multiple sandboxes by indices from the list below.
Run `hc sandbox list` to see the sandboxes or `hc sandbox run --help` for more information."
            );
            crate::save::list(std::env::current_dir()?, false)?;
            Err(std::io::Error::other(
                "Multiple sandboxes found, please specify which to run.",
            ))
        }
    }

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
            num_sandboxes: NonZeroUsize::new(1).unwrap(),
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
    use anyhow;
    use holochain_conductor_api::conductor::paths::{ConfigFilePath, ConfigRootPath};
    use std::fs;
    use tempfile;

    #[test]
    fn test_existing_is_empty() {
        // Test when both all is false and indices is empty
        let existing = Existing {
            all: false,
            indices: vec![],
        };
        assert!(existing.is_empty());

        // Test when all is true
        let existing = Existing {
            all: true,
            indices: vec![],
        };
        assert!(!existing.is_empty());

        // Test when indices is not empty
        let existing = Existing {
            all: false,
            indices: vec![0],
        };
        assert!(!existing.is_empty());
    }

    #[test]
    fn test_existing_load_invalid_path() -> anyhow::Result<()> {
        // Test loading all sandboxes
        let result = Existing {
            all: true,
            indices: vec![],
        }
        .load(PathBuf::from("invalid_path"));

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_existing_load_no_conductor_config_file() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        let result = Existing {
            all: true,
            indices: vec![],
        }
        .load(test_dir.to_path_buf());

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_existing_load_empty_file() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        crate::save::save(test_dir.to_path_buf(), vec![])?;

        let result = Existing {
            all: true,
            indices: vec![],
        }
        .load(test_dir.to_path_buf());

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_existing_load_all_valid_sandboxes() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths to .hc file
        crate::save::save(test_dir.to_path_buf(), vec![config_path1, config_path2])?;

        // Test loading all sandboxes
        let paths = Existing {
            all: true,
            indices: vec![],
        }
        .load(test_dir.to_path_buf())?;

        // Verify the loaded paths
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], sandbox1);
        assert_eq!(paths[1], sandbox2);

        Ok(())
    }

    #[test]
    fn test_existing_load_all_with_invalid_sandboxes() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config file only for sandbox1
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());

        // Save the paths to .hc file
        crate::save::save(test_dir.to_path_buf(), vec![config_path1, config_path2])?;

        // Test loading all sandboxes
        let existing = Existing {
            all: true,
            indices: vec![],
        };
        let paths = existing.load(test_dir.to_path_buf())?;

        // Verify the loaded paths (only valid ones should be returned)
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], sandbox1);

        Ok(())
    }

    #[test]
    fn test_existing_load_specific_indices() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        let sandbox3 = test_dir.join("sandbox3");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;
        fs::create_dir_all(&sandbox3)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        let config_path3 = ConfigRootPath::from(sandbox3.clone());
        let config_file_path3 = ConfigFilePath::from(config_path3.clone());
        fs::create_dir_all(config_file_path3.as_ref().parent().unwrap())?;
        fs::write(config_file_path3.as_ref(), "dummy config")?;

        // Save the paths to .hc file
        crate::save::save(
            test_dir.to_path_buf(),
            vec![config_path1, config_path2, config_path3],
        )?;

        // Test loading specific sandboxes by indices
        let existing = Existing {
            all: false,
            indices: vec![0, 2],
        };
        let paths = existing.load(test_dir.to_path_buf())?;

        // Verify the loaded paths
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], sandbox1);
        assert_eq!(paths[1], sandbox3);

        // Test loading specific sandboxes by indices
        let existing = Existing {
            all: false,
            indices: vec![1],
        };
        let paths = existing.load(test_dir.to_path_buf())?;

        // Verify the loaded paths
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], sandbox2);

        Ok(())
    }

    #[test]
    fn test_existing_load_specific_indices_with_invalid_sandboxes() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config file only for sandbox1
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());

        // Save the paths to .hc file
        crate::save::save(test_dir.to_path_buf(), vec![config_path1, config_path2])?;

        // Test loading specific sandboxes by indices
        let existing = Existing {
            all: false,
            indices: vec![0, 1],
        };

        // This should return an error because one of the sandboxes is invalid
        let result = existing.load(test_dir.to_path_buf());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_existing_load_duplicate_indices() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directory
        let sandbox1 = test_dir.join("sandbox1");
        fs::create_dir_all(&sandbox1)?;

        // Create conductor config file
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        // write the path twice to .hc file
        crate::save::save(
            test_dir.to_path_buf(),
            vec![config_path1.clone(), config_path1],
        )?;

        // Test loading with duplicate indices
        let existing = Existing {
            all: false,
            indices: vec![0, 0],
        };

        let result = existing.load(test_dir.to_path_buf())?;
        assert_eq!(result.len(), 1);

        // Test loading with duplicate indices
        let existing = Existing {
            all: false,
            indices: vec![0, 1, 0],
        };
        let result = existing.load(test_dir.to_path_buf())?;
        assert_eq!(result.len(), 2);

        Ok(())
    }

    #[test]
    fn test_existing_load_out_of_range_indices() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directory
        let sandbox1 = test_dir.join("sandbox1");
        fs::create_dir_all(&sandbox1)?;

        // Create conductor config file
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        // Save the path to .hc file
        crate::save::save(test_dir.to_path_buf(), vec![config_path1])?;

        // Test loading with an out-of-range index
        let existing = Existing {
            all: false,
            indices: vec![1], // Only index 0 exists
        };

        let result = existing.load(test_dir.to_path_buf());

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_existing_load_single_sandbox() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directory
        let sandbox1 = test_dir.join("sandbox1");
        fs::create_dir_all(&sandbox1)?;

        // Create conductor config file
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        // Save the path to .hc file
        crate::save::save(test_dir.to_path_buf(), vec![config_path1])?;

        // Test loading with empty indices (should use the single sandbox)
        let existing = Existing {
            all: false,
            indices: vec![],
        };

        let paths = existing.load(test_dir.to_path_buf())?;

        // Should return the single sandbox
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], sandbox1);

        Ok(())
    }

    #[test]
    fn test_existing_load_single_invalid_sandbox() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directory
        let sandbox1 = test_dir.join("sandbox1");
        fs::create_dir_all(&sandbox1)?;

        // Create path but don't create the config file
        let config_path1 = ConfigRootPath::from(sandbox1.clone());

        // Save the path to .hc file
        crate::save::save(test_dir.to_path_buf(), vec![config_path1])?;

        // Test loading with empty indices (should fail because the single sandbox is invalid)
        let existing = Existing {
            all: false,
            indices: vec![],
        };

        let result = existing.load(test_dir.to_path_buf());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_existing_load_multiple_sandboxes_no_indices() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths to .hc file
        crate::save::save(test_dir.to_path_buf(), vec![config_path1, config_path2])?;

        // Test loading with empty indices (should fail because there are multiple sandboxes)
        let existing = Existing {
            all: false,
            indices: vec![],
        };

        let result = existing.load(test_dir.to_path_buf());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_existing_load_no_sandboxes() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();

        // Test loading none
        let result = Existing {
            all: false,
            indices: vec![],
        }
        .load(test_dir.to_path_buf());
        assert!(result.is_err());

        // Test loading all
        let result = Existing {
            all: true,
            indices: vec![],
        }
        .load(test_dir.to_path_buf());

        assert!(result.is_err());

        Ok(())
    }
}
