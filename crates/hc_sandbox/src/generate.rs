//! Helpers for generating new directories and [`ConductorConfig`].

use std::path::PathBuf;

use holochain_conductor_api::conductor::ConductorConfig;
use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;

use crate::config::create_config;
use crate::config::write_config;

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
    let dir = generate_directory(root, directory)?;
    let mut config = create_config(dir.clone());
    config.network = network;
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
    let dir = generate_directory(root, directory)?;
    let config = config.unwrap_or_else(|| create_config(dir.clone()));
    write_config(dir.clone(), &config);
    Ok(dir)
}

/// Generate a new directory structure for a sandbox.
pub fn generate_directory(
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let mut dir = root.unwrap_or_else(std::env::temp_dir);
    let directory = directory.unwrap_or_else(|| nanoid::nanoid!().into());
    dir.push(directory);
    std::fs::create_dir(&dir)?;
    let mut keystore_dir = dir.clone();
    keystore_dir.push("keystore");
    std::fs::create_dir(keystore_dir)?;
    Ok(dir)
}
