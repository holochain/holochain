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

use std::process::Command;

// Useful to have this public when using this as a library.
pub use holochain_cli_bundle as hc_bundle;
use holochain_cli_sandbox as hc_sandbox;
use lazy_static::lazy_static;
use clap::{Parser, Subcommand, crate_version};

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
            .map(|s| format!("  {}\t  Run \"hc {} help\" to see its help", s, s))
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
    ["hc-web-app", "hc-dna", "hc-app", "hc-sandbox"]
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
pub enum CliSubcommand {
    /// Work with DNA bundles.
    Dna(hc_bundle::HcDnaBundle),
    /// Work with hApp bundles.
    App(hc_bundle::HcAppBundle),
    /// Work with web-hApp bundles.
    WebApp(hc_bundle::HcWebAppBundle),
    /// Work with sandboxed environments for testing and development.
    Sandbox(hc_sandbox::HcSandbox),
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
            CliSubcommand::External(args) => {
                let command_suffix = args.first().expect("Missing subcommand name");
                Command::new(format!("hc-{}", command_suffix))
                    .args(&args[1..])
                    .status()
                    .expect("Failed to run external subcommand");
            }
        }
        Ok(())
    }
}
