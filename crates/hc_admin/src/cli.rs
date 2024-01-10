#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use clap::{Parser, Subcommand};
use holochain_types::prelude::{AppManifest, DnaManifest, ValidatedDnaManifest};
use holochain_types::web_app::WebAppManifest;
use holochain_util::ffs;
use mr_bundle::{Location, Manifest};
use std::path::Path;
use std::path::PathBuf;

use crate::error::HcBundleResult;

/// Work with Holochain DNA bundles.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct HcAdmin {
    /// The websocket port to connect to
    pub port: u16,
}

impl HcAdmin {
    /// Run the command.
    pub async fn run(self) -> anyhow::Result<()> {
        println!("{}", self.port);
        Ok(())
    }
}
