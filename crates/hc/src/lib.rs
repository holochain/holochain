#![warn(missing_docs)]

//! A library and CLI to help create, run and interact with holochain conductor setups.
//! **Warning this is still WIP and subject to change**
//! There's probably a few bugs. If you find one please open an [issue](https://github.com/holochain/holochain/issues)
//! or make a PR.
//!
//! ## CLI
//! The `hc` CLI makes it easy to run a dna that you are working on
//! or someone has sent you.
//! It has been designed to use sensible defaults but still give you
//! the configurability when that's required.
//! Setups are stored in tmp directories by default and the paths are
//! persisted in a `.hc` file which is created wherever you are using
//! the CLI.
//! ### Install
//! #### Requirements
//! - [Rust](https://rustup.rs/)
//! - [Holochain](https://github.com/holochain/holochain) binary on the path
//! #### Building
//! From github:
//! ```shell
//! cargo install holochain_cli --git https://github.com/holochain/holochain
//! ```
//! From the holochain repo:
//! ```shell
//! cargo install --path crates/hc
//! ```
//! ### Common usage
//! The best place to start is:
//! ```shell
//! hc -h
//! ```
//! This will be more up to date then this readme.
//! #### Run
//! This command can be used to generate and run conductor setups.
//! ```shell
//! hc run -h
//! # or shorter
//! hc r -h
//! ```
//!  In a folder with where your `my-dna.dna` is you can generate and run
//!  a new setup with:
//! ```shell
//! hc r
//! ```
//! If you have already created a setup previously then it will be reused
//! (usually cleared on reboots).
//! #### Generate
//! Generates new conductor setups and installs apps / dnas.
//! ```shell
//! hc generate
//! # or shorter
//! hc g
//! ```
//! For example this will generate 5 setups with app ids set to `my-app`
//! using the `elemental-chat.dna` from the current directory with a quic
//! network setup to localhost.
//! _You don't need to specify dnas when they are in the directory._
//! ```shell
//!  hc gen -a "my-app" -n 5 ./elemental-chat.dna network quic
//! ```
//! You can also generate and run in the same command:
//! (Notice the number of conductors and dna path must come before the gen sub-command).
//! ```shell
//!  hc r -n 5 ./elemental-chat.dna gen -a "my-app" network quic
//! ```
//! #### Call
//! Allows calling the [`AdminRequest`](https://docs.rs/holochain_conductor_api/latest/holochain_conductor_api/enum.AdminRequest.html) api.
//! If the conductors are not already running they
//! will be run to make the call.
//!
//! ```shell
//! hc call list-cells
//! ```
//! #### List and Clean
//! These commands allow you to list the persisted setups
//! in the current directory (from the`.hc`) file.
//! You can use the index from:
//! ```shell
//! hc list
//! ```
//! Output:
//! ```shell
//! hc-sandbox:
//! Setups contained in `.hc`
//! 0: /tmp/KOXgKVLBVvoxe8iKD4iSS
//! 1: /tmp/m8VHwwt93Uh-nF-vr6nf6
//! 2: /tmp/t6adQomMLI5risj8K2Tsd
//! ```
//! To then call or run an individual setup (or subset):
//!
//! ```shell
//! hc r -i=0,2
//! ```
//! You can clean up these setups with:
//! ```shell
//! hc clean 0 2
//! # Or clean all
//! hc clean
//! ```
//! ## Library
//! This crate can also be used as a library so you can create more
//! complex setups / admin calls.
//! See the docs:
//! ```shell
//! cargo doc --open
//! ```
//! and the examples.

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
            0 => format!(""),
            _ => format!(
                r#"
CLI EXTENSIONS:
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
    /// Work with Web-hApp bundles
    WebApp(hc_bundle::HcWebAppBundle),
    /// Work with sandboxed environments for testing and development
    Sandbox(hc_sandbox::HcSandbox),
    /// Allow redirect of external subcommands (like hc-scaffold and hc-launch)
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
