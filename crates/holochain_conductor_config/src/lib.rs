pub mod cmds;
pub mod config;
pub mod generate;
pub mod ports;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use cmds::{Network, NetworkCmd};

/// Print a message with `hc-conductor-config: ` prepended and ANSI colors.
#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => ({
        use ansi_term::Color::*;
        print!("{} ", Blue.bold().paint("hc-conductor-config:"));
        println!($($arg)*);
    })
}

#[derive(Debug, Parser, Clone)]
pub struct ConductorConfigCli {
    /// Collect the lair passphrase by reading stdin to the end.
    #[arg(short, long)]
    piped: bool,

    #[command(subcommand)]
    command: ConductorConfigCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum ConductorConfigCmd {
    Create {
        /// Add an optional network config.
        #[command(subcommand)]
        network: Option<NetworkCmd>,

        /// Set a root directory for conductor sandboxes to be placed into.
        /// Defaults to the system's temp directory.
        /// This directory must already exist.
        #[arg(long)]
        root: Option<PathBuf>,

        /// Specify the root directory name where the configurations will
        /// be stored, by default, a random string will be the default.
        #[arg(short, long)]
        directrory: Option<PathBuf>,

        /// Launch Holochain with an embedded lair server instead of a standalone process.
        /// Use this option to run the sandboxed conductors when you don't have access to the lair binary.
        #[arg(long)]
        in_process_lair: bool,
    },
}

impl ConductorConfigCli {
    pub async fn run(self) -> anyhow::Result<()> {
        match self.command {
            ConductorConfigCmd::Create {
                network,
                root,
                directrory,
                in_process_lair,
            } => {
                holochain_util::pw::pw_set_piped(self.piped);
                msg!("Creating configurations");
                let network = Network::to_kitsune(&NetworkCmd::as_inner(&network)).await;
                let path = crate::generate::generate(network, root, directrory, in_process_lair)?;
                msg!("Created {:?}", path);
            }
        }
        Ok(())
    }
}
