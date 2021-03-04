#![warn(missing_docs)]

//! A library and CLI to help create, run and interact with holochain conductor sandboxes.
//! **Warning this is still WIP and subject to change**
//! There's probably a few bugs. If you find one please open an [issue](https://github.com/holochain/holochain/issues)
//! or make a PR.
//!
//! ## CLI
//! The `hc sandbox` CLI makes it easy to run a dna that you are working on
//! or someone has sent you.
//! It has been designed to use sensible defaults but still give you
//! the configurability when that's required.
//! Sandboxes are stored in tmp directories by default and the paths are
//! persisted in a `.hcXXX` file which is created wherever you are using
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
//! cargo install --path crates/hcYYY
//! ```
//! ### Common usage
//! The best place to start is:
//! ```shell
//! hc sandbox -h
//! ```
//! This will be more up to date then this readme.
//! #### Run
//! This command can be used to generate and run conductor sandboxes.
//! ```shell
//! hc sandbox run -h
//! # or shorter
//! hc sandbox r -h
//! ```
//!  In a folder with where your `my-dna.dna` is you can generate and run
//!  a new sandbox with:
//! ```shell
//! hc sandbox r
//! ```
//! If you have already created a sandbox previously then it will be reused
//! (usually cleared on reboots).
//! #### Generate
//! Generates new conductor sandboxes and installs apps / dnas.
//! ```shell
//! hc sandbox generate
//! # or shorter
//! hc sandbox g
//! ```
//! For example this will generate 5 sandboxes with app ids set to `my-app`
//! using the `elemental-chat.dna` from the current directory with a quic
//! network sandbox to localhost.
//! _You don't need to specify dnas when they are in the directory._
//! ```shell
//! hc sandbox gen -a "my-app" -n 5 ./elemental-chat.dna network quic
//! ```
//! You can also generate and run in the same command:
//! (Notice the number of conductors and dna path must come before the gen sub-command).
//! ```shell
//! hc sandbox r -n 5 ./elemental-chat.dna gen -a "my-app" network quic
//! ```
//! #### Call
//! Allows calling the [`AdminRequest`] api.
//! If the conductors are not already running they
//! will be run to make the call.
//!
//! ```shell
//! hc sandbox call list-cells
//! ```
//! #### List and Clean
//! These commands allow you to list the persisted sandboxes
//! in the current directory (from the`.hcXXX`) file.
//! You can use the index from:
//! ```shell
//! hc sandbox list
//! ```
//! Output:
//! ```shell
//! hc-sandbox:
//! Sandboxes contained in `.hcXXX`
//! 0: /tmp/KOXgKVLBVvoxe8iKD4iSS
//! 1: /tmp/m8VHwwt93Uh-nF-vr6nf6
//! 2: /tmp/t6adQomMLI5risj8K2Tsd
//! ```
//! To then call or run an individual sandbox (or subset):
//!
//! ```shell
//! hc sandbox r -i=0,2
//! ```
//! You can clean up these sandboxes with:
//! ```shell
//! hc sandbox clean 0 2
//! # Or clean all
//! hc sandbox clean
//! ```
//! ## Library
//! This crate can also be used as a library so you can create more
//! complex sandboxes / admin calls.
//! See the docs:
//! ```shell
//! cargo doc --open
//! ```
//! and the examples.

#![allow(deprecated)]

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

pub mod calls;
pub mod cli;
#[doc(hidden)]
pub mod cmds;
pub mod config;
pub mod dna;
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
        Self::from_sandbox_with_bin_path(&Path::new(Self::HOLOCHAIN_PATH), sandbox_path).await
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
