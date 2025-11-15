use std::path::PathBuf;

use clap::{Parser, Subcommand};
use holochain_trace::Output;

pub use holochain_cli_sandbox::calls::Call as AdminCallArgs;
pub use holochain_cli_sandbox::zome_call::{ZomeCall, ZomeCallAuth};

#[derive(Debug, Parser)]
#[command(name = "hc-client", about = "CLI utilities for interacting with running Holochain conductors")]
pub struct ClientCli {
    /// Path to the `holochain` binary used to launch conductors if needed.
    #[arg(
        short = 'H',
        long,
        env = "HC_HOLOCHAIN_PATH",
        default_value = "holochain"
    )]
    pub holochain_path: PathBuf,

    /// Force admin ports to specific values when spawning conductors automatically.
    #[arg(short, long, value_delimiter = ',')]
    pub force_admin_ports: Vec<u16>,

    /// Structured log output option for spawned conductors.
    #[arg(long, default_value_t = Output::Log)]
    pub structured: Output,

    #[command(subcommand)]
    pub command: ClientCommand,
}

#[derive(Debug, Subcommand)]
pub enum ClientCommand {
    /// Make a call to a conductor's admin interface.
    Call(AdminCallArgs),
    /// Create and authorize credentials for making zome calls.
    ZomeCallAuth(ZomeCallAuth),
    /// Make a zome call to a running app.
    ZomeCall(ZomeCall),
}
