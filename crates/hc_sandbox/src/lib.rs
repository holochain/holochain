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

pub use ports::force_admin_port;

/// Print a message with `hc-sandbox: ` prepended
/// and ANSI colors.
macro_rules! msg {
    ($($arg:tt)*) => ({
        use ansi_term::Color::*;
        print!("{} ", Blue.bold().paint("hc-sandbox:"));
        println!($($arg)*);
    })
}

pub mod bundles;
pub mod cli;
#[doc(hidden)]
pub mod cmds;
pub mod run;
pub mod sandbox;
pub mod save;
pub use cli::HcSandbox;

mod ports;
