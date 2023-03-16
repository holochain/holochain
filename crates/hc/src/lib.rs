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
use structopt::{lazy_static::lazy_static, StructOpt};

mod external_subcommands;

lazy_static! {
    static ref HELP: &'static str = {
        let extensions = external_subcommands::list_external_subcommands()
            .into_iter()
            .map(|s| format!("    hc {}\t  Run \"hc {} help\" to see its help", s, s))
            .collect::<Vec<String>>()
            .join("\n");

        let extensions_str = match extensions.len() {
            0 => String::from(""),
            _ => format!(
                r#"
EXTENSIONS:
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

/// Describes all the possible CLI arguments for `hc`, including external subcommands like `hc-scaffold`
#[allow(clippy::large_enum_variant)]
#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::InferSubcommands)]
#[structopt(long_about = *HELP)]
pub enum Opt {
    /// Work with DNA bundles
    Dna(hc_bundle::HcDnaBundle),
    /// Work with hApp bundles
    App(hc_bundle::HcAppBundle),
    /// Work with web-hApp bundles
    WebApp(hc_bundle::HcWebAppBundle),
    /// Work with sandboxed environments for testing and development
    Sandbox(hc_sandbox::HcSandbox),
    /// Allow redirect of external subcommands (like `hc-scaffold` and `hc-launch`)
    #[structopt(external_subcommand)]
    External(Vec<String>),
}

impl Opt {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Dna(cmd) => cmd.run().await?,
            Self::App(cmd) => cmd.run().await?,
            Self::WebApp(cmd) => cmd.run().await?,
            Self::Sandbox(cmd) => cmd.run().await?,
            Self::External(args) => {
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
