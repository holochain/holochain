//! Helpers for generating new directories and `ConductorConfig`.

use holochain_conductor_api::conductor::{
    paths::{ConfigRootPath, KeystorePath},
    NetworkConfig,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(feature = "unstable-dpki")]
use holochain_conductor_api::conductor::DpkiConfig;

use crate::config::create_config;
use crate::config::write_config;
use crate::msg;
use crate::ports::set_admin_port;

/// Generate configurations
/// This creates a directory containing a `ConductorConfig`,
/// a keystore, and a database root directory.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    network: Option<NetworkConfig>,
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
    in_process_lair: bool,
    admin_port: u16,
    #[cfg(feature = "unstable-dpki")] no_dpki: bool,
    #[cfg(feature = "unstable-dpki")] dpki_network_seed: Option<String>,
    #[cfg(feature = "chc")] chc_url: Option<url2::Url2>,
) -> anyhow::Result<ConfigRootPath> {
    let dir = generate_config_directory(root, directory)?;

    let lair_connection_url = if !in_process_lair {
        let keystore_path = KeystorePath::try_from(dir.is_also_data_root_path())?;
        let passphrase = holochain_util::pw::pw_get()?;
        let conn_url = init_lair(&keystore_path, passphrase)?;

        msg!("Connection URL? {:?}", conn_url);
        Some(conn_url)
    } else {
        None
    };

    let mut config = create_config(dir.clone(), lair_connection_url)?;
    config.network = network.unwrap_or_default();
    #[cfg(feature = "chc")]
    {
        config.chc_url = chc_url;
    }
    #[cfg(feature = "unstable-dpki")]
    if no_dpki {
        config.dpki = DpkiConfig::disabled();
    } else if let Some(network_seed) = dpki_network_seed {
        config.dpki.network_seed = network_seed;
    }
    set_admin_port(&mut config, admin_port);
    let path = write_config(dir.clone(), &config)?;
    msg!("Config {:?}", config);
    msg!(
        "Created directory at: {} {} It has also been saved to a file called `.hc` in your current working directory.",
        ansi_term::Style::new()
            .bold()
            .underline()
            .on(ansi_term::Color::Fixed(254))
            .fg(ansi_term::Color::Fixed(4))
            .paint(dir.display().to_string()),
        ansi_term::Style::new()
            .bold()
            .paint("Keep this path to rerun the same sandbox.")
    );
    msg!("Created config at {}", path.display());
    Ok(dir)
}

/// Generate a new directory structure for configurations
pub fn generate_config_directory(
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> anyhow::Result<ConfigRootPath> {
    let mut dir = root.unwrap_or_else(std::env::temp_dir);
    let directory = directory.unwrap_or_else(|| nanoid::nanoid!().into());
    dir.push(directory);
    std::fs::create_dir(&dir)?;

    Ok(dir.into())
}

pub fn init_lair(
    dir: &KeystorePath,
    passphrase: Arc<Mutex<sodoken::LockedArray>>,
) -> anyhow::Result<url2::Url2> {
    match init_lair_inner(dir, passphrase) {
        Ok(url) => Ok(url),
        Err(err) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to execute 'lair-keystore init': {:?}", err),
        )
        .into()),
    }
}

pub(crate) fn init_lair_inner(
    dir: &KeystorePath,
    passphrase: Arc<Mutex<sodoken::LockedArray>>,
) -> anyhow::Result<url2::Url2> {
    let mut cmd = std::process::Command::new("lair-keystore");

    cmd.args(["init", "--piped"])
        .current_dir(dir.as_ref())
        .stdin(std::process::Stdio::piped());

    let mut proc = cmd.spawn()?;
    let mut stdin = proc.stdin.take().unwrap();

    use std::io::Write;
    stdin.write_all(&passphrase.lock().unwrap().lock())?;
    stdin.flush()?;
    drop(stdin);

    if !proc.wait()?.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "LairInitFail").into());
    }
    let conf = dir.as_ref().join("lair-keystore-config.yaml");

    let conf = std::fs::read(conf)?;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Conf {
        connection_url: url2::Url2,
    }

    let conf: Conf = serde_yaml::from_slice(&conf)?;

    Ok(conf.connection_url)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::read_config;

    use anyhow::Context;
    use holochain_conductor_api::{
        conductor::{paths::KEYSTORE_DIRECTORY, ConductorConfig, KeystoreConfig},
        AdminInterfaceConfig, InterfaceDriver,
    };
    use holochain_types::websocket::AllowedOrigins;
    use tempfile::tempdir;

    #[test]
    fn test_generate_creates_default_config_file() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let root = Some(temp_dir.path().to_path_buf());
        let directory = Some("test-config".into());

        let config_root = generate(
            None,
            root,
            directory,
            true,
            0,
            #[cfg(feature = "unstable-dpki")]
            false,
            #[cfg(feature = "unstable-dpki")]
            None,
            #[cfg(feature = "chc")]
            None,
        )?;

        assert!(config_root.as_path().exists());
        assert!(config_root.as_path().is_dir());

        let config_file = config_root.as_path().join("conductor-config.yaml");
        assert!(config_file.exists());
        assert!(config_file.is_file());

        let config = read_config(config_root.clone())
            .context("Failed to read config")?
            .expect("Config file does not exist in config root");

        let expected_config = ConductorConfig {
            data_root_path: Some(config_root.is_also_data_root_path()),
            network: NetworkConfig::default(),
            keystore: KeystoreConfig::LairServerInProc {
                lair_root: Some(config_root.join(KEYSTORE_DIRECTORY).into()),
            },
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port: 0,
                    allowed_origins: AllowedOrigins::Any,
                },
            }]),
            ..Default::default()
        };

        assert_eq!(config, expected_config);

        Ok(())
    }

    #[test]
    fn test_generate_with_custom_network() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let root = Some(temp_dir.path().to_path_buf());
        let directory = Some("test-config".into());

        let network_config = NetworkConfig {
            bootstrap_url: url2::url2!("test-boot:"),
            signal_url: url2::url2!("test-sig:"),
            ..Default::default()
        };

        let config_root = generate(
            Some(network_config.clone()),
            root,
            directory,
            true,
            0,
            #[cfg(feature = "unstable-dpki")]
            false,
            #[cfg(feature = "unstable-dpki")]
            None,
            #[cfg(feature = "chc")]
            None,
        )?;

        assert!(config_root.as_path().exists());
        assert!(config_root.as_path().is_dir());

        let config_file = config_root.as_path().join("conductor-config.yaml");
        assert!(config_file.exists());
        assert!(config_file.is_file());

        let config = read_config(config_root.clone())
            .context("Failed to read config")?
            .expect("Config file does not exist in config root");

        let expected_config = ConductorConfig {
            data_root_path: Some(config_root.is_also_data_root_path()),
            network: network_config,
            keystore: KeystoreConfig::LairServerInProc {
                lair_root: Some(config_root.join(KEYSTORE_DIRECTORY).into()),
            },
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port: 0,
                    allowed_origins: AllowedOrigins::Any,
                },
            }]),
            ..Default::default()
        };

        assert_eq!(config, expected_config);

        Ok(())
    }
}
