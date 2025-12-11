#![warn(missing_docs)]
#![doc = r#"Utilities for connecting to running Holochain conductors.

The `holochain_cli_client` crate exposes shared helpers and a command-line
interface for issuing admin API requests and signed zome calls against an
already-running conductor."#]

/// Print a message with `hc-client:` prefix and ANSI colors.
macro_rules! msg {
    ($($arg:tt)*) => ({
        use ansi_term::Color::*;
        print!("{} ", Blue.bold().paint("hc-client:"));
        println!($($arg)*);
    })
}

pub(crate) use msg;

pub mod calls;
/// CLI entry points for the `hc-client` binary.
pub mod cli;
pub mod zome_call;

pub use cli::HcClient;
