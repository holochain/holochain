//! Functions for persisting and loading ConductorState to/from the normalized database.
//!
//! This bridges between `holochain_data` operations and the `ConductorState` type.

use super::error::{ConductorError, ConductorResult};
use crate::conductor::state::{AppInterfaceConfig, AppInterfaceId};
use crate::conductor::state::{ConductorState, ConductorStateTag};
use holochain_conductor_api::signal_subscription::SignalSubscription;
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
    let interface_models = db
        .get_all_app_interfaces()
        .await
        .map_err(ConductorError::other)?;

    let mut app_interfaces = HashMap::new();
    for model in interface_models {
        // Convert model to AppInterfaceConfig
        let driver = model.to_driver().map_err(ConductorError::other)?;

        // Get signal subscriptions for this interface
        let subs_data = db
            .get_signal_subscriptions(model.port, model.id.as_deref())
            .await
            .map_err(ConductorError::other)?;

        let mut signal_subscriptions = HashMap::new();
        for (app_id, filters_blob) in subs_data {
            let subscription: SignalSubscription =
                serde_json::from_slice(&filters_blob).map_err(|e| {
                    ConductorError::other(format!(
                        "Failed to deserialize signal subscription: {}",
                        e
                    ))
                })?;
            signal_subscriptions.insert(InstalledAppId::from(app_id), subscription);
        }

        let config = AppInterfaceConfig {
            signal_subscriptions,
            installed_app_id: model.installed_app_id,
            driver,
        };

        // Reconstruct AppInterfaceId
        let interface_id = if model.port == 0 {
            // Port 0 case - must have an ID
            let id = model
                .id
                .ok_or_else(|| ConductorError::other("Port 0 interface missing ID"))?;
            // Use private fields via a constructor we need to add
            // For now, we'll use `new` and manually update the id
            // This is a limitation - we may need to expose a constructor
            AppInterfaceId::from_parts(0, Some(id))
        } else {
            AppInterfaceId::new(model.port as u16)
        };
        app_interfaces.insert(interface_id, config);
    }

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

    // Save all app interfaces
    for (interface_id, config) in &state.app_interfaces {
        let model = holochain_data::conductor::AppInterfaceModel::from_driver(
            &config.driver,
            config.installed_app_id.as_ref().map(|id| id.to_string()),
        )
        .map_err(ConductorError::other)?;

        db.put_app_interface(
            interface_id.port() as i64,
            interface_id.id().as_deref(),
            &model,
        )
        .await
        .map_err(ConductorError::other)?;

        // Clear existing signal subscriptions for this interface
        db.delete_signal_subscriptions(interface_id.port() as i64, interface_id.id().as_deref())
            .await
            .map_err(ConductorError::other)?;

        // Save signal subscriptions
        for (app_id, subscription) in &config.signal_subscriptions {
            let filters_blob = serde_json::to_vec(subscription).map_err(|e| {
                ConductorError::other(format!("Failed to serialize signal subscription: {}", e))
            })?;
            db.put_signal_subscription(
                interface_id.port() as i64,
                interface_id.id().as_deref(),
                app_id.as_ref(),
                &filters_blob,
            )
            .await
            .map_err(ConductorError::other)?;
        }
    }

    // TODO: Implement deletion of apps and interfaces that are no longer in the state
    // This requires using a read connection to get existing data, then using the write connection to delete
    // For now, we'll skip this step as it requires restructuring the API

    Ok(())
}
