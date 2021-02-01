//! Normalized, validated representation of the App Manifest.
//!
//! The versioned manifest structs are designed to be deserialized from YAML,
//! and so they contain various optional fields. They are not validated, and
//! may contain various invalid combinations of data. In contrast, these types
//! are structured to ensure validity, and are used internally by Holochain.

use crate::app::app_manifest::current::{DnaLocation, DnaVersionSpec};
use crate::prelude::{CellNick, YamlProperties};
use std::collections::HashMap;

use super::error::{AppManifestError, AppManifestResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppManifestValidated {
    /// Name of the App. This may be used as the installed_app_id.
    name: String,

    /// The Cell manifests that make up this app.
    cells: HashMap<CellNick, CellManifestValidated>,
}

impl AppManifestValidated {
    /// Constructor with internal consistency check.
    ///
    /// NB: never make this struct's fields public. This constructor should be
    /// the only way to instantiate this type.
    pub fn new(
        name: String,
        cells: HashMap<CellNick, CellManifestValidated>,
    ) -> AppManifestResult<Self> {
        for (nick, cell) in cells.iter() {
            if let CellManifestValidated::Disabled { clone_limit } = cell {
                if *clone_limit == 0 {
                    return Err(AppManifestError::InvalidStrategyDisabled(nick.to_owned()));
                }
            }
        }
        Ok(AppManifestValidated { name, cells })
    }
}

/// Rules to determine if and how a Cell will be created for this Dna
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellManifestValidated {
    /// Always create a new Cell when installing this App
    Create {
        clone_limit: u32,
        deferred: bool,
        location: DnaLocation,
        properties: Option<YamlProperties>,
        uuid: Option<String>, // TODO: use UUID
        version: Option<DnaVersionSpec>,
    },
    /// Always create a new Cell when installing the App,
    /// and use a unique UUID to ensure a distinct DHT network
    CreateClone {
        clone_limit: u32,
        deferred: bool,
        location: DnaLocation,
        properties: Option<YamlProperties>,
        version: Option<DnaVersionSpec>,
    },
    /// Require that a Cell is already installed which matches the DNA version
    /// spec, and which has an Agent that's associated with this App's agent
    /// via DPKI. If no such Cell exists, *app installation fails*.
    UseExisting {
        clone_limit: u32,
        deferred: bool,
        version: DnaVersionSpec,
    },
    /// Try `UseExisting`, and if that fails, fallback to `Create`
    CreateIfNotExists {
        clone_limit: u32,
        deferred: bool,
        location: DnaLocation,
        properties: Option<YamlProperties>,
        uuid: Option<String>, // TODO: use UUID
        version: DnaVersionSpec,
    },
    /// Disallow provisioning altogether. In this case, we expect
    /// `clone_limit > 0`: otherwise, no Cells will ever be created.
    Disabled { clone_limit: u32 },
}
