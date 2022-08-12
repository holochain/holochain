//! Helpers for generating new directories and [`ConductorConfig`].

use std::path::PathBuf;

use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;
use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;

use crate::config::create_config;
use crate::config::write_config;
use crate::ports::random_admin_port;

/// Generate a new sandbox.
/// This creates a directory and a [`ConductorConfig`]
/// from an optional network.
/// The root directory and inner directory
/// (where this sandbox will be created) can be overridden.
/// For example `my_root_dir/this_sandbox_dir/`
pub fn generate(
    network: Option<KitsuneP2pConfig>,
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let (dir, con_url) = generate_directory(root, directory)?;

    let mut config = create_config(dir.clone(), con_url);
    config.network = network;
    random_admin_port(&mut config);
    let path = write_config(dir.clone(), &config);
    msg!("Config {:?}", config);
    msg!(
        "Created directory at: {} {}",
        ansi_term::Style::new()
            .bold()
            .underline()
            .on(ansi_term::Color::Fixed(254))
            .fg(ansi_term::Color::Fixed(4))
            .paint(dir.display().to_string()),
        ansi_term::Style::new()
            .bold()
            .paint("Keep this path to rerun the same sandbox")
    );
    msg!("Created config at {}", path.display());
    Ok(dir)
}

/// Generate a new sandbox from a full config.
pub fn generate_with_config(
    config: Option<ConductorConfig>,
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let (dir, con_url) = generate_directory(root, directory)?;
    let config = config.unwrap_or_else(|| {
        let mut config = create_config(dir.clone(), con_url.clone());
        config.keystore = KeystoreConfig::LairServer {
            connection_url: con_url,
        };
        config
    });
    write_config(dir.clone(), &config);
    Ok(dir)
}

/// Generate a new directory structure for a sandbox.
pub fn generate_directory(
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> anyhow::Result<(PathBuf, url2::Url2)> {
    let passphrase = holochain_util::pw::pw_get()?;

    let mut dir = root.unwrap_or_else(std::env::temp_dir);
    let directory = directory.unwrap_or_else(|| nanoid::nanoid!().into());
    dir.push(directory);
    std::fs::create_dir(&dir)?;
    let mut keystore_dir = dir.clone();
    keystore_dir.push("keystore");
    std::fs::create_dir(&keystore_dir)?;

    let con_url = init_lair(&keystore_dir, passphrase)?;

    Ok((dir, con_url))
}

pub(crate) fn init_lair(
    dir: &std::path::Path,
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
    dir: &std::path::Path,
    passphrase: sodoken::BufRead,
) -> anyhow::Result<url2::Url2> {
    let mut cmd = std::process::Command::new("lair-keystore");

    cmd.args(["init", "--piped"])
        .current_dir(dir)
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

    let mut conf = std::path::PathBuf::from(dir);
    conf.push("lair-keystore-config.yaml");

    let conf = std::fs::read(&conf)?;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Conf {
        connection_url: url2::Url2,
    }

    let conf: Conf = serde_yaml::from_slice(&conf)?;

    Ok(conf.connection_url)
}
