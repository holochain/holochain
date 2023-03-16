#![warn(missing_docs)]

//! # holochain_cli_sandbox
//!
//! A library and CLI to help create, run, and interact with sandboxed Holochain conductor environments,
//! for testing and development purposes.
//! **Warning: this is still WIP and subject to change**
//! There's probably a few bugs. If you find one please open an [issue](https://github.com/holochain/holochain/issues)
//! or make a PR.
//! 
//! While this crate can be compiled into an executable, it can also be used as a library so you can create more
//! complex sandboxes / admin calls.
//! See the docs:
//!
//! ```shell
//! cargo doc --open
//! ```
//!
//! and the examples.

use std::path::Path;
use std::path::PathBuf;

use holochain_conductor_api::{AdminRequest, AdminResponse};
use holochain_websocket::WebsocketResult;
use holochain_websocket::WebsocketSender;
use ports::get_admin_api;

pub use ports::force_admin_port;

/// Print a msg with `hc-sandbox: ` pre-pended
/// and ansi colors.
macro_rules! msg {
    ($($arg:tt)*) => ({
        use ansi_term::Color::*;
        print!("{} ", Blue.bold().paint("hc-sandbox:"));
        println!($($arg)*);
    })
}

pub mod bundles;
pub mod calls;
pub mod cli;
#[doc(hidden)]
pub mod cmds;
pub mod config;
pub mod generate;
pub mod run;
pub mod sandbox;
pub mod save;
pub use cli::HcSandbox;

mod ports;

/// An active connection to a running conductor.
pub struct CmdRunner {
    client: WebsocketSender,
}

impl CmdRunner {
    const HOLOCHAIN_PATH: &'static str = "holochain";
    /// Create a new connection for calling admin interface commands.
    /// Panics if admin port fails to connect.
    pub async fn new(port: u16) -> Self {
        Self::try_new(port)
            .await
            .expect("Failed to create CmdRunner because admin port failed to connect")
    }

    /// Create a new connection for calling admin interface commands.
    pub async fn try_new(port: u16) -> WebsocketResult<Self> {
        let client = get_admin_api(port).await?;
        Ok(Self { client })
    }

    /// Create a command runner from a sandbox path.
    /// This expects holochain to be on the path.
    pub async fn from_sandbox(
        sandbox_path: PathBuf,
    ) -> anyhow::Result<(Self, tokio::process::Child)> {
        Self::from_sandbox_with_bin_path(Path::new(Self::HOLOCHAIN_PATH), sandbox_path).await
    }

    /// Create a command runner from a sandbox path and
    /// set the path to the holochain binary.
    pub async fn from_sandbox_with_bin_path(
        holochain_bin_path: &Path,
        sandbox_path: PathBuf,
    ) -> anyhow::Result<(Self, tokio::process::Child)> {
        let conductor = run::run_async(holochain_bin_path, sandbox_path, None).await?;
        let cmd = CmdRunner::try_new(conductor.0).await?;
        Ok((cmd, conductor.1))
    }

    /// Make an Admin request to this conductor.
    pub async fn command(&mut self, cmd: AdminRequest) -> anyhow::Result<AdminResponse> {
        let response: Result<AdminResponse, _> = self.client.request(cmd).await;
        Ok(response?)
    }
}

#[macro_export]
/// Expect that an enum matches a variant and panic if it doesn't.
macro_rules! expect_variant {
    ($var:expr => $variant:path, $error_msg:expr) => {
        match $var {
            $variant(v) => v,
            _ => panic!(format!("{}: Expected {} but got {:?}", $error_msg, stringify!($variant), $var)),
        }
    };
    ($var:expr => $variant:path) => {
        expect_variant!($var => $variant, "")
    };
}

#[macro_export]
/// Expect that an enum matches a variant and return an error if it doesn't.
macro_rules! expect_match {
    ($var:expr => $variant:path, $error_msg:expr) => {
        match $var {
            $variant(v) => v,
            _ => anyhow::bail!("{}: Expected {} but got {:?}", $error_msg, stringify!($variant), $var),
        }
    };
    ($var:expr => $variant:path) => {
        expect_variant!($var => $variant, "")
    };
}
