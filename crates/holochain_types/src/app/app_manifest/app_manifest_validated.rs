//! Normalized, validated representation of the App Manifest.
//!
//! The versioned manifest structs are designed to be deserialized from YAML,
//! and so they contain various optional fields. They are not validated, and
//! may contain various invalid combinations of data. In contrast, these types
//! are structured to ensure validity, and are used internally by Holochain.

use holo_hash::DnaHashB64;
use holochain_zome_types::DnaModifiersOpt;

use super::error::{AppManifestError, AppManifestResult};
use crate::app::app_manifest::current::DnaLocation;
use crate::prelude::RoleName;
use std::collections::HashMap;

/// Normalized, validated representation of the App Manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppManifestValidated {
    /// Name of the App. This may be used as the installed_app_id.
    pub(in crate::app) name: String,

    /// The role descriptions that make up this app.
    pub(in crate::app) roles: HashMap<RoleName, AppRoleManifestValidated>,
}

impl AppManifestValidated {
    /// Constructor with internal consistency checks.
    ///
    /// NB: never make this struct's fields public. This constructor should be
    /// the only way to instantiate this type.
    pub(in crate::app) fn new(
        name: String,
        roles: HashMap<RoleName, AppRoleManifestValidated>,
    ) -> AppManifestResult<Self> {
        for (role_name, role) in roles.iter() {
            if let AppRoleManifestValidated::Disabled { clone_limit, .. } = role {
                if *clone_limit == 0 {
                    return Err(AppManifestError::InvalidStrategyDisabled(
                        role_name.to_owned(),
                    ));
                }
            }
        }
        Ok(AppManifestValidated { name, roles })
    }
}

/// Rules to determine if and how a Cell will be created for this Dna
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppRoleManifestValidated {
    /// Always create a new Cell when installing this App
    Create {
        clone_limit: u32,
        deferred: bool,
        location: DnaLocation,
        modifiers: DnaModifiersOpt,
        version: Option<DnaHashB64>,
    },
    /// Always create a new Cell when installing the App,
    /// and use a unique network seed to ensure a distinct DHT network
    CreateClone {
        clone_limit: u32,
        deferred: bool,
        location: DnaLocation,
        modifiers: DnaModifiersOpt,
        version: Option<DnaHashB64>,
    },
    /// Require that a Cell is already installed which matches the DNA version
    /// spec, and which has an Agent that's associated with this App's agent
    /// via DPKI. If no such Cell exists, *app installation fails*.
    UseExisting {
        clone_limit: u32,
        deferred: bool,
        version: DnaHashB64,
    },
    /// Try `UseExisting`, and if that fails, fallback to `Create`
    CreateIfNotExists {
        clone_limit: u32,
        deferred: bool,
        location: DnaLocation,
        modifiers: DnaModifiersOpt,
        version: DnaHashB64,
    },
    /// Disallow provisioning altogether. In this case, we expect
    /// `clone_limit > 0`: otherwise, no cells will ever be created.
    Disabled {
        version: DnaHashB64,
        clone_limit: u32,
    },
}
