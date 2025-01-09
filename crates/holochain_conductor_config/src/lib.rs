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
