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
            .get_signal_subscriptions(
                model.port,
                model
                    .id
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.as_str()),
            )
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
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ConductorError::other("Port 0 interface missing ID"))?;
            // Use private fields via a constructor we need to add
            // For now, we'll use `new` and manually update the id
            // This is a limitation - we may need to expose a constructor
            AppInterfaceId::from_parts(0, Some(id))
        } else {
            // For non-zero ports, ignore the empty string id from database
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
            interface_id.id().as_deref().or(Some("")),
            &model,
        )
        .await
        .map_err(ConductorError::other)?;

        // Clear existing signal subscriptions for this interface
        db.delete_signal_subscriptions(
            interface_id.port() as i64,
            interface_id.id().as_deref().or(Some("")),
        )
        .await
        .map_err(ConductorError::other)?;

        // Save signal subscriptions
        for (app_id, subscription) in &config.signal_subscriptions {
            let filters_blob = serde_json::to_vec(subscription).map_err(|e| {
                ConductorError::other(format!("Failed to serialize signal subscription: {}", e))
            })?;
            db.put_signal_subscription(
                interface_id.port() as i64,
                interface_id.id().as_deref().or(Some("")),
                app_id.as_ref(),
                &filters_blob,
            )
            .await
            .map_err(ConductorError::other)?;
        }
    }

    // Delete apps and interfaces that are no longer in the state
    let db_read: &holochain_data::DbRead<holochain_data::kind::Conductor> = db.as_ref();

    // Get all existing app IDs from database
    let existing_apps = db_read
        .get_all_installed_apps()
        .await
        .map_err(ConductorError::other)?;
    let existing_app_ids: Vec<String> = existing_apps
        .into_iter()
        .map(|(app_id, _, _)| app_id)
        .collect();

    // Delete apps that are no longer in state
    for app_id in existing_app_ids {
        if !state
            .installed_apps()
            .contains_key(&InstalledAppId::from(app_id.clone()))
        {
            db.delete_installed_app(&app_id)
                .await
                .map_err(ConductorError::other)?;
        }
    }

    // Get all existing app interfaces from database
    let existing_interfaces = db_read
        .get_all_app_interfaces()
        .await
        .map_err(ConductorError::other)?;

    // Delete interfaces that are no longer in state
    for model in existing_interfaces {
        let interface_id = if model.port == 0 {
            let id = model
                .id
                .clone()
                .ok_or_else(|| ConductorError::other("Port 0 interface missing ID"))?;
            AppInterfaceId::from_parts(0, Some(id))
        } else {
            AppInterfaceId::new(model.port as u16)
        };

        if !state.app_interfaces.contains_key(&interface_id) {
            db.delete_app_interface(model.port, model.id.as_deref())
                .await
                .map_err(ConductorError::other)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conductor::state::AppInterfaceConfig;
    use holochain_data::setup_holochain_data;
    use holochain_types::websocket::AllowedOrigins;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_state_persistence_round_trip() {
        // Create a temporary database
        let tmpdir = tempfile::tempdir().unwrap();
        let db_write = setup_holochain_data(
            tmpdir.path(),
            holochain_data::kind::Conductor,
            holochain_data::HolochainDataConfig::default(),
        )
        .await
        .unwrap();
        let db_read = db_write.as_ref();

        // Create a test state
        let tag = ConductorStateTag(Arc::from("test-conductor"));
        let installed_apps = InstalledAppMap::new();
        let app_interfaces = HashMap::new();
        let state = ConductorState::from_parts(tag.clone(), installed_apps, app_interfaces);

        // Save the state
        save_conductor_state(&db_write, &state).await.unwrap();

        // Load the state back
        let loaded_state = load_conductor_state(&db_read).await.unwrap().unwrap();

        // Verify the tag matches
        assert_eq!(loaded_state.tag().0.as_ref(), "test-conductor");
        assert_eq!(loaded_state.installed_apps().len(), 0);
        assert_eq!(loaded_state.app_interfaces.len(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_app_interface_persistence() {
        // Create a temporary database
        let tmpdir = tempfile::tempdir().unwrap();
        let db_write = setup_holochain_data(
            tmpdir.path(),
            holochain_data::kind::Conductor,
            holochain_data::HolochainDataConfig::default(),
        )
        .await
        .unwrap();
        let db_read = db_write.as_ref();

        // Create a test state with an app interface
        let tag = ConductorStateTag(Arc::from("test-conductor"));
        let installed_apps = InstalledAppMap::new();

        let mut app_interfaces = HashMap::new();
        let interface_config =
            AppInterfaceConfig::websocket(12345, None, AllowedOrigins::Any, None);
        let interface_id = AppInterfaceId::new(12345);
        app_interfaces.insert(interface_id, interface_config);

        let state = ConductorState::from_parts(tag, installed_apps, app_interfaces);

        // Save the state
        save_conductor_state(&db_write, &state).await.unwrap();

        // Load the state back
        let loaded_state = load_conductor_state(&db_read).await.unwrap().unwrap();

        // Verify the interface was persisted
        assert_eq!(loaded_state.app_interfaces.len(), 1);
        let loaded_interface = loaded_state.app_interfaces.values().next().unwrap();
        assert_eq!(loaded_interface.driver.port(), 12345);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_deletion_of_stale_interfaces() {
        // Create a temporary database
        let tmpdir = tempfile::tempdir().unwrap();
        let db_write = setup_holochain_data(
            tmpdir.path(),
            holochain_data::kind::Conductor,
            holochain_data::HolochainDataConfig::default(),
        )
        .await
        .unwrap();
        let db_read = db_write.as_ref();

        // Create initial state with two interfaces
        let tag = ConductorStateTag(Arc::from("test-conductor"));
        let installed_apps = InstalledAppMap::new();

        let mut app_interfaces = HashMap::new();
        let interface1 = AppInterfaceConfig::websocket(12345, None, AllowedOrigins::Any, None);
        let interface2 = AppInterfaceConfig::websocket(12346, None, AllowedOrigins::Any, None);
        app_interfaces.insert(AppInterfaceId::new(12345), interface1);
        app_interfaces.insert(AppInterfaceId::new(12346), interface2);

        let state = ConductorState::from_parts(tag.clone(), installed_apps.clone(), app_interfaces);
        save_conductor_state(&db_write, &state).await.unwrap();

        // Now create a new state with only one interface
        let mut app_interfaces = HashMap::new();
        let interface1 = AppInterfaceConfig::websocket(12345, None, AllowedOrigins::Any, None);
        app_interfaces.insert(AppInterfaceId::new(12345), interface1);
        let new_state = ConductorState::from_parts(tag, installed_apps, app_interfaces);

        // Save the new state (should delete interface 12346)
        save_conductor_state(&db_write, &new_state).await.unwrap();

        // Load and verify only one interface remains
        let loaded_state = load_conductor_state(&db_read).await.unwrap().unwrap();
        assert_eq!(loaded_state.app_interfaces.len(), 1);
        assert!(loaded_state
            .app_interfaces
            .contains_key(&AppInterfaceId::new(12345)));
    }
}
