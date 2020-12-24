//! Helper functions for interacting with the user when running a Conductor
//! with the --interactive flag

use holochain_conductor_api::conductor::ConductorConfigError;

use crate::conductor::config::ConductorConfig;
use crate::conductor::error::ConductorError;
use crate::conductor::error::ConductorResult;
use crate::conductor::paths::ConfigFilePath;
use std::path::Path;

/// Prompt the user to answer Y or N.
///
/// `prompt` will be printed as the question to answer.
/// if `default_yes` is Some(true), entering a blank line equates to Y
/// if `default_yes` is Some(false), entering a blank line equates to N
/// if `default_yes` is None, Y or N must be explicitly entered, anything else is invalid
///
/// Returns true for Y, false for N
pub fn ask_yn(prompt: String, default_yes: Option<bool>) -> std::io::Result<bool> {
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

/// Prompts user to enter an LMDB environment path
pub fn prompt_for_environment_dir(path: &Path) -> std::io::Result<()> {
    let prompt = format!(
        "There is no database environment set at the path specified ({})\nWould you like to create one now?",
        path.display()
    );
    if ask_yn(prompt, Some(true))? {
        std::fs::create_dir_all(path)?;
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Cannot continue without LMDB environment created.",
        ))
    }
}

/// If config_path is Some, attempt to load the config from that path, and return error if file not found
/// If config_path is None, attempt to load config from default path, and offer to create config if file not found
pub fn load_config_or_prompt_for_default(
    config_path: ConfigFilePath,
) -> ConductorResult<Option<ConductorConfig>> {
    ConductorConfig::load_yaml(config_path.as_ref()).map(Some).or_else(|err| {
        if let ConductorConfigError::ConfigMissing(_) = err {
            let prompt = format!(
                "There is no conductor config YAML file at the path specified ({})\nWould you like to create a default config file at this location?",
                config_path
            );
            if ask_yn(prompt, Some(true))? {
                let config = save_default_config_yaml(config_path.as_ref())?;
                println!("Conductor config written.");
                Ok(Some(config))
            } else {
                Ok(None)
            }
        } else {
            Err(err.into())
        }
    })
}

/// Save the default [ConductorConfig] to `path`
fn save_default_config_yaml(path: &Path) -> ConductorResult<ConductorConfig> {
    let dir = path.parent().ok_or_else(|| {
        ConductorError::ConfigError(format!("Bad path for conductor config: {}", path.display()))
    })?;
    std::fs::create_dir_all(dir)?;
    let default = ConductorConfig::default();
    let content_yaml = serde_yaml::to_string(&default)?;
    std::fs::write(path, content_yaml)?;
    Ok(default)
}

#[cfg(test)]
mod tests {
    use super::save_default_config_yaml;
    use crate::conductor::config::ConductorConfig;
    use tempdir::TempDir;

    #[test]
    fn test_save_default_config() {
        let tmp = TempDir::new("test").unwrap();
        let config_path = tmp.path().join("config.yaml");
        save_default_config_yaml(&config_path).unwrap();
        let config = ConductorConfig::load_yaml(config_path.as_ref()).unwrap();
        assert_eq!(config, ConductorConfig::default());
    }
}
