//! Helper functions for interacting with the user when running a Conductor
//! with the --interactive flag

use holochain_conductor_api::conductor::ConductorConfigError;

use crate::conductor::config::ConductorConfig;
use crate::conductor::error::ConductorError;
use crate::conductor::error::ConductorResult;
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
                Some(answer) if input.is_empty() => return Ok(answer),
                _ => println!("Invalid answer."),
            }
        }
    }
}

/// Prompts user to enter a database path
pub fn prompt_for_database_dir(path: &Path) -> std::io::Result<()> {
    let prompt = format!(
        "There is no database at the path specified ({})\nWould you like to create one now?",
        path.display()
    );
    if ask_yn(prompt, Some(true))? {
        std::fs::create_dir_all(path)?;
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Cannot continue without database.",
        ))
    }
}