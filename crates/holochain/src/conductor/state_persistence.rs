//! Functions for persisting and loading ConductorState to/from the normalized database.
//!
//! This bridges between `holochain_data` operations and the `ConductorState` type.

use super::error::{ConductorError, ConductorResult};
use crate::conductor::state::{ConductorState, ConductorStateTag};
use holochain_types::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Load ConductorState from normalized holochain_data tables
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn load_conductor_state(
    db: &holochain_data::DbRead<holochain_data::kind::Conductor>,
) -> ConductorResult<Option<ConductorState>> {
    // Get conductor tag
    let tag = db
        .get_conductor_tag()
        .await
        .map_err(ConductorError::other)?;
    let tag = match tag {
        Some(tag) => ConductorStateTag(Arc::from(tag.as_str())),
        None => return Ok(None),
    };

    // Get all installed apps
    let installed_apps_vec = db
        .get_all_installed_apps()
        .await
        .map_err(ConductorError::other)?;

    let mut installed_apps = InstalledAppMap::new();
    for (app_id, app_common, status) in installed_apps_vec {
        let mut installed_app = InstalledApp::new_fresh(app_common);
        installed_app.status = status;
        installed_apps.insert(InstalledAppId::from(app_id.clone()), installed_app);
    }

    // Get all app interfaces
    // TODO: Implement get_all_app_interfaces in holochain_data
    let app_interfaces = HashMap::new();

    Ok(Some(ConductorState::from_parts(
        tag,
        installed_apps,
        app_interfaces,
    )))
}

/// Save ConductorState to normalized holochain_data tables
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn save_conductor_state(
    db: &holochain_data::DbWrite<holochain_data::kind::Conductor>,
    state: &ConductorState,
) -> ConductorResult<()> {
    // Save conductor tag
    db.set_conductor_tag(&state.tag().0)
        .await
        .map_err(ConductorError::other)?;

    // Save all installed apps
    for (app_id, installed_app) in state.installed_apps() {
        db.put_installed_app(app_id.as_ref(), installed_app, &installed_app.status)
            .await
            .map_err(ConductorError::other)?;
    }

    // TODO: Save app interfaces
    // TODO: Delete apps that are no longer in the state

    Ok(())
}
