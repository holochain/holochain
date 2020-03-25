use holochain_2020::conductor::{
    api::ExternalConductorApi,
    config::ConductorConfig,
    error::{ConductorError, ConductorResult},
    interface::{channel::ChannelInterface, Interface},
    paths::ConfigFilePath,
    Conductor,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use structopt::StructOpt;
use sx_types::observability::{self, Output};
use tokio::sync::{mpsc, RwLock};
use tracing::*;

#[derive(Debug, StructOpt)]
#[structopt(name = "holochain", about = "The Holochain Conductor.")]
struct Opt {
    #[structopt(
        long,
        help = "Outputs structured json from logging:
    - None: No logging at all (fastest)
    - Log: Output logs to stdout with spans (human readable)
    - Compact: Same as Log but with less information
    - Json: Output logs as structured json (machine readable)
    ",
        default_value = "Log"
    )]
    structured: Output,

    #[structopt(short = "c")]
    config_path: Option<PathBuf>,

    #[structopt(short = "i", long)]
    interactive: bool,
}

// enum ConfigOpt {

// }

// impl TryFrom<Opt> for ConfigOpt {
//     type Error = String;
//     try_from(opt: Opt) -> Self {
//         match (opt.config, opt.config_or_default) {
//             (Some(c), None) => ConfigOpt()
//         }
//     }
// }

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();
    observability::init_fmt(opt.structured).expect("Failed to start contextual logging");
    debug!("observability initialized");

    let config_path: ConfigFilePath = opt.config_path.map(Into::into).unwrap_or_default();
    debug!("config_path: {}", config_path);

    let config: ConductorConfig = if opt.interactive {
        load_config_or_prompt_for_default(config_path)
            .expect("Could not load conductor config")
            .unwrap_or_else(|| {
                println!("Cannot continue without configuration");
                std::process::exit(1);
            })
    } else {
        ConductorConfig::load_toml(config_path).expect("Could not load conductor config")
    };

    let env_path = PathBuf::from(config.environment_path.clone());

    if opt.interactive && !env_path.is_dir() {
        prompt_for_environment_dir(&env_path).expect("Couldn't auto-create environment dir");
    }

    let conductor: Conductor = Conductor::build()
        .from_config(config)
        .await
        .expect("Could not initialize Conductor from configuration");

    let lock = Arc::new(RwLock::new(conductor));
    let api = ExternalConductorApi::new(lock);

    interface_example(api).await;
}

/// Prompt the user to answer Y or N.
///
/// `prompt` will be printed as the question to answer.
/// if `default_yes` is Some(true), entering a blank line equates to Y
/// if `default_yes` is Some(false), entering a blank line equates to N
/// if `default_yes` is None, Y or N must be explicitly entered, anything else is invalid
///
/// Returns true for Y, false for N
fn ask_yn(prompt: String, default_yes: Option<bool>) -> ConductorResult<bool> {
    let choices = match default_yes {
        Some(true) => "[Y/n]",
        Some(false) => "[y/N]",
        None => "[y/n]",
    };
    loop {
        let mut input = String::new();
        println!("{} {}", prompt, choices);
        std::io::stdin().read_line(&mut input)?;
        let input = input.to_ascii_lowercase();
        let input = input.trim_end();

        if input == "y" {
            return Ok(true);
        } else if input == "n" {
            return Ok(false);
        } else {
            match default_yes {
                Some(answer) if input == "" => return Ok(answer),
                _ => println!("Invalid answer."),
            }
        }
    }
}

fn prompt_for_environment_dir(path: &Path) -> ConductorResult<()> {
    let prompt = format!(
        "There is no database environment set at the path specified ({})\nWould you like to create one now?", path.display()
    );
    if ask_yn(prompt, Some(true))? {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

/// If config_path is Some, attempt to load the config from that path, and return error if file not found
/// If config_path is None, attempt to load config from default path, and offer to create config if file not found
fn load_config_or_prompt_for_default(
    config_path: ConfigFilePath,
) -> ConductorResult<Option<ConductorConfig>> {
    ConductorConfig::load_toml(config_path.clone()).map(Some).or_else(|err| {
        if let ConductorError::ConfigMissing(_) = err {
            let prompt = format!(
                "There is no conductor config TOML file at the path specified ({})\nWould you like to create a default config file at this location?",
                config_path
            );
            if ask_yn(prompt, Some(true))? {
                let config = save_default_config_toml(config_path.as_ref())?;
                println!("Conductor config written.");
                Ok(Some(config))
            } else {
                Ok(None)
            }
        } else {
            Err(err)
        }
    })
}

/// Save the default [ConductorConfig] to `path`
fn save_default_config_toml(path: &Path) -> ConductorResult<ConductorConfig> {
    let dir = path.parent().ok_or_else(|| {
        ConductorError::ConfigError(format!("Bad path for conductor config: {}", path.display()))
    })?;
    std::fs::create_dir_all(dir)?;
    let default = ConductorConfig::default();
    let content_toml = toml::to_string(&default)?;
    std::fs::write(path, content_toml)?;
    Ok(default)
}

async fn interface_example(api: ExternalConductorApi) {
    let (mut tx_dummy, rx_dummy) = mpsc::channel(100);

    let interface_fut = ChannelInterface::new(rx_dummy).spawn(api);
    let driver_fut = async move {
        for _ in 0..50 as u32 {
            debug!("sending dummy msg");
            tx_dummy.send(true).await.unwrap();
        }
        tx_dummy.send(false).await.unwrap();
    };
    tokio::join!(interface_fut, driver_fut);
}

#[cfg(test)]
mod tests {

    use crate::save_default_config_toml;
    use holochain_2020::conductor::config::ConductorConfig;
    use tempdir::TempDir;

    #[test]
    fn test_save_default_config() {
        let tmp = TempDir::new("test").unwrap();
        let config_path = tmp.path().join("config.toml");
        save_default_config_toml(&config_path).unwrap();
        let config = ConductorConfig::load_toml(config_path.into()).unwrap();
        assert_eq!(config, ConductorConfig::default());
    }

}
