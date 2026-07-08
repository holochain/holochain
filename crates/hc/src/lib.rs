#![warn(missing_docs)]

//! A library and CLI to help create, run, and interact with Holochain conductor setups.
//! **Warning this is still WIP and subject to change**
//! There's probably a few bugs. If you find one please open an [issue](https://github.com/holochain/holochain/issues)
//! or make a PR.
//!
//! ## CLI
//!
//! The `hc` CLI makes it easy to create, modify, and run hApps that
//! you are working on or someone has sent you.
//! It has been designed to use sensible defaults but still give you
//! the configurability when that's required.
//!
//! Setups are stored in tmp directories by default and the paths are
//! persisted in a `.hc` file which is created wherever you are using
//! the CLI.

// Useful to have this public when using this as a library.
use clap::{crate_version, Parser, Subcommand};
pub use holochain_cli_bundle as hc_bundle;
use holochain_cli_client as hc_client;
use holochain_cli_sandbox as hc_sandbox;
use lazy_static::lazy_static;
use std::process::Command;

mod external_subcommands;

// TODO: change this so it inherits clap's formatting.
// Clap 3 and 4 format helptext using colours and bold/underline respectively.
// https://github.com/clap-rs/clap/pull/4765 introduces the ability to style your own help text
// using a library like `color_print`.
// https://github.com/clap-rs/clap/issues/4786 requests that the styler's built-in helper methods
// be exposed to consumers, thereby allowing us to durably make our styling consistent
// with whatever clap's happens to be at the moment.
// I'd prefer the latter approach, if it lands.
lazy_static! {
    static ref HELP: &'static str = {
        let extensions = external_subcommands::list_external_subcommands()
            .into_iter()
            .map(|s| format!("  {s}\t  Run \"hc {s} help\" to see its help"))
            .collect::<Vec<String>>()
            .join("\n");

        let extensions_str = match extensions.len() {
            0 => String::from(""),
            _ => format!(
                r#"
Extensions:
{extensions}"#
            ),
        };

        let s = format!(
            r#"Holochain CLI

Work with DNA, hApp and web-hApp bundle files, set up sandbox environments for testing and development purposes, make direct admin calls to running conductors, and more.
{extensions_str}"#
        );
        Box::leak(s.into_boxed_str())
    };
}

fn builtin_commands() -> Vec<String> {
    ["hc-web-app", "hc-dna", "hc-app", "hc-sandbox", "hc-client"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// The main entry-point for the command.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Parser)]
#[command(about = *HELP, infer_subcommands = true, allow_external_subcommands = true, version = crate_version!())]
pub struct Cli {
    /// The `hc` subcommand to run.
    #[command(subcommand)]
    pub subcommand: CliSubcommand,
}

/// Describes all the possible CLI arguments for `hc`, including external subcommands like `hc-scaffold`.
#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum CliSubcommand {
    /// Work with DNA bundles.
    Dna(hc_bundle::HcDnaBundle),
    /// Work with hApp bundles.
    App(hc_bundle::HcAppBundle),
    /// Work with web-hApp bundles.
    WebApp(hc_bundle::HcWebAppBundle),
    /// Work with sandboxed environments for testing and development.
    Sandbox(hc_sandbox::HcSandbox),
    /// Connect to and interact with running Holochain conductors.
    Client(hc_client::HcClient),
    /// Allow redirect of external subcommands (like `hc-scaffold` and `hc-launch`).
    #[command(external_subcommand)]
    External(Vec<String>),
}

impl CliSubcommand {
    /// Run this command.
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            CliSubcommand::App(cmd) => cmd.run().await?,
            CliSubcommand::Dna(cmd) => cmd.run().await?,
            CliSubcommand::WebApp(cmd) => cmd.run().await?,
            CliSubcommand::Sandbox(cmd) => cmd.run().await?,
            CliSubcommand::Client(cmd) => cmd.run().await?,
            CliSubcommand::External(args) => {
                let command_suffix = args.first().expect("Missing subcommand name");
                let exe_name = format!("hc-{command_suffix}");

                match Command::new(&exe_name).args(&args[1..]).status() {
                    Ok(status) => {
                        std::process::exit(status.code().unwrap_or(1));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                        eprintln!(
                            "error: `{command_suffix}' is not a recognized internal hc subcommand, nor is '{exe_name}' an external command on your PATH."
                        );

                        std::process::exit(1);
                    }
                    Err(other_err) => {
                        eprintln!("error: Failed to execute '{exe_name}': {other_err}");
                        std::process::exit(1);
                    }
                }
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::prelude::*;

    #[test]
    fn test_help_flag() {
        let mut cmd = Command::cargo_bin("hc").unwrap();

        cmd.arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:"));
    }

    #[test]
    fn test_no_subcommand() {
        let mut cmd = Command::cargo_bin("hc").unwrap();

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Usage:"));
    }

    #[test]
    fn test_predefined_subcommand() {
        let mut cmd = Command::cargo_bin("hc").unwrap();

        cmd.arg("sandbox")
            .assert()
            .failure()
            .stderr(predicate::str::contains("Work with sandboxed environments"));
    }

    #[test]
    fn test_undefined_subcommand() {
        let mut cmd = Command::cargo_bin("hc").unwrap();

        cmd.arg("blah")
            .assert()
            .failure()
            .stderr(predicate::str::contains("not a recognized internal hc subcommand"));
    }
}
