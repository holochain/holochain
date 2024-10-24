//! Helpers for generating new directories and [`ConductorConfig`].

use std::path::PathBuf;

use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::conductor::paths::KeystorePath;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::conductor::DpkiConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;
use kitsune_p2p_types::config::KitsuneP2pConfig;

use crate::config::create_config;
use crate::config::write_config;
use crate::ports::random_admin_port;

/// Generate a new sandbox.
/// This creates a directory containing a [`ConductorConfig`],
/// a keystore, and a database.
/// The root directory and inner directory
/// (where this sandbox will be created) can be overridden.
/// For example `my_root_dir/this_sandbox_dir/`
pub fn generate(
    network: Option<KitsuneP2pConfig>,
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
    in_process_lair: bool,
    no_dpki: bool,
    dpki_network_seed: Option<String>,
    #[cfg(feature = "chc")] chc_url: Option<url2::Url2>,
) -> anyhow::Result<ConfigRootPath> {
    let (dir, con_url) = generate_directory(root, directory, !in_process_lair)?;

    let mut config = create_config(dir.clone(), con_url)?;
    config.network = network.unwrap_or_else(KitsuneP2pConfig::testing);
    #[cfg(feature = "chc")]
    {
        config.chc_url = chc_url;
    }
    if no_dpki {
        config.dpki = DpkiConfig::disabled();
    } else if let Some(network_seed) = dpki_network_seed {
        config.dpki.network_seed = network_seed;
    }
    random_admin_port(&mut config);
    let path = write_config(dir.clone(), &config);
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

/// Generate a new sandbox from a full config.
pub fn generate_with_config(
    config: Option<ConductorConfig>,
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> anyhow::Result<ConfigRootPath> {
    let (dir, con_url) = generate_directory(root, directory, true)?;
    let config = match config {
        Some(config) => config,
        None => {
            let mut config = create_config(dir.clone(), con_url.clone())?;
            config.keystore = KeystoreConfig::LairServer {
                connection_url: con_url.expect(
                    "Lair should have been initialised but did not get a connection URL for it",
                ),
            };
            config
        }
    };
    write_config(dir.clone(), &config);
    Ok(dir)
}

/// Generate a new directory structure for a sandbox.
pub fn generate_directory(
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
    initialise_lair: bool,
) -> anyhow::Result<(ConfigRootPath, Option<url2::Url2>)> {
    let passphrase = holochain_util::pw::pw_get()?;

    let mut dir = root.unwrap_or_else(std::env::temp_dir);
    let directory = directory.unwrap_or_else(|| nanoid::nanoid!().into());
    dir.push(directory);
    std::fs::create_dir(&dir)?;

    let config_root_path = ConfigRootPath::from(dir);
    let keystore_path = KeystorePath::try_from(config_root_path.is_also_data_root_path())?;

    let con_url = if initialise_lair {
        Some(init_lair(&keystore_path, passphrase)?)
    } else {
        None
    };

    Ok((config_root_path, con_url))
}

pub(crate) fn init_lair(
    dir: &KeystorePath,
    passphrase: sodoken::BufRead,
) -> anyhow::Result<url2::Url2> {
    match init_lair_inner(dir, passphrase) {
        Err(err) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to execute 'lair-keystore init': {:?}", err),
        )
        .into()),
        Ok(url) => Ok(url),
    }
}

pub(crate) fn init_lair_inner(
    dir: &KeystorePath,
    passphrase: sodoken::BufRead,
) -> anyhow::Result<url2::Url2> {
    let mut cmd = std::process::Command::new("lair-keystore");

    cmd.args(["init", "--piped"])
        .current_dir(dir.as_ref())
        .stdin(std::process::Stdio::piped());

    let mut proc = cmd.spawn()?;
    let mut stdin = proc.stdin.take().unwrap();

    use std::io::Write;
    stdin.write_all(&passphrase.read_lock()[..])?;
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
