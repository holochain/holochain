use std::path::PathBuf;

use clap::{Parser, Subcommand};
use holochain_cli_sandbox::config::{create_config, write_config};
use holochain_conductor_api::conductor::paths::{ConfigRootPath, KeystorePath};
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::{AdminInterfaceConfig, InterfaceDriver};
use holochain_types::websocket::AllowedOrigins;
use kitsune_p2p_types::config::KitsuneP2pConfig;

macro_rules! msg {
    ($($arg:tt)*) => ({
        use ansi_term::Color::*;
        print!("{} ", Blue.bold().paint("hc-sandbox:"));
        println!($($arg)*);
    })
}

#[derive(Debug, Parser, Clone)]
pub struct ConductorConfigCli {
    #[command(subcommand)]
    command: ConductorConfigCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum ConductorConfigCmd {
    Create {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long)]
        in_process_lair: bool,
    },
}

impl ConductorConfigCli {
    pub async fn run(self) -> anyhow::Result<()> {
        match self.command {
            ConductorConfigCmd::Create {
                path,
                in_process_lair,
            } => {
                let _ = generate(path, in_process_lair)?;
            }
        }
        Ok(())
    }
}

/// Generate a new sandbox.
/// This creates a directory containing a [`ConductorConfig`],
/// a keystore, and a database.
/// The root directory and inner directory
/// (where this sandbox will be created) can be overridden.
/// For example `my_root_dir/this_sandbox_dir/`
pub fn generate(path: PathBuf, in_process_lair: bool) -> anyhow::Result<ConfigRootPath> {
    let (dir, con_url) = generate_directory(path, in_process_lair)?;

    let mut config = create_config(dir.clone(), con_url)?;
    config.network = KitsuneP2pConfig::mem();
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

/// Generate a new directory structure for a sandbox.
pub fn generate_directory(
    path: PathBuf,
    in_process_lair: bool,
) -> anyhow::Result<(ConfigRootPath, Option<url2::Url2>)> {
    let passphrase = holochain_util::pw::pw_get()?;

    std::fs::create_dir_all(&path)?;

    let config_root_path = ConfigRootPath::from(path);
    let keystore_path = KeystorePath::try_from(config_root_path.is_also_data_root_path())?;

    let con_url = if in_process_lair {
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
        Ok(url) => Ok(url),
        Err(err) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to execute 'lair-keystore init': {:?}", err),
        )
        .into()),
    }
}

fn init_lair_inner(dir: &KeystorePath, passphrase: sodoken::BufRead) -> anyhow::Result<url2::Url2> {
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

fn random_admin_port(config: &mut ConductorConfig) {
    match config.admin_interfaces.as_mut().and_then(|i| i.first_mut()) {
        Some(AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port, .. },
        }) => {
            if *port != 0 {
                *port = 0;
            }
        }
        None => {
            let port = 0;
            config.admin_interfaces = Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port,
                    allowed_origins: AllowedOrigins::Any,
                },
            }]);
        }
    }
}
