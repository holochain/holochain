use std::path::PathBuf;

use holochain_p2p::kitsune_p2p::KitsuneP2pConfig;

use crate::config::create_config;
use crate::config::write_config;

pub async fn create(
    network: Option<KitsuneP2pConfig>,
    root: Option<PathBuf>,
    directory: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let mut dir = root.unwrap_or_else(std::env::temp_dir);
    let directory = directory.unwrap_or_else(|| nanoid::nanoid!().into());
    dir.push(directory);
    std::fs::create_dir(dir.clone())?;
    let mut keystore_dir = dir.clone();
    keystore_dir.push("keystore");
    std::fs::create_dir(keystore_dir)?;
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
            .paint("Keep this path to rerun the same setup")
    );
    msg!("Created config at {}", path.display());
    Ok(dir)
}

pub async fn create_default() -> anyhow::Result<PathBuf> {
    create(None, None, None).await
}
