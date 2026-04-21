//! Helpers for converting between the in-memory [`ConductorState`] type and
//! the [`ConductorStateSnapshot`] storage representation used by the conductor store.
//!
//! Persistence itself (loading a consistent snapshot, atomic read/modify/write)
//! lives on [`holochain_state::conductor::ConductorStore`]; this module just
//! handles the type conversion either side of those operations.

use super::error::{ConductorError, ConductorResult};
use crate::conductor::state::{AppInterfaceConfig, AppInterfaceId};
use crate::conductor::state::{ConductorState, ConductorStateTag};
use holochain_conductor_api::signal_subscription::SignalSubscription;
use holochain_state::conductor::ConductorStateSnapshot;
use holochain_types::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Build a [`ConductorState`] from a persisted snapshot.
pub fn snapshot_to_state(snapshot: ConductorStateSnapshot) -> ConductorResult<ConductorState> {
    let tag = ConductorStateTag(Arc::from(snapshot.tag.as_str()));

    let mut installed_apps = InstalledAppMap::new();
    for (app_id, app_common, status) in snapshot.installed_apps {
        let mut installed_app = InstalledApp::new_fresh(app_common);
        installed_app.status = status;
        installed_apps.insert(InstalledAppId::from(app_id), installed_app);
    }

    let mut app_interfaces = HashMap::new();
    for (model, subs_data) in snapshot.app_interfaces {
        let driver = model.to_driver().map_err(ConductorError::other)?;

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
            installed_app_id: model.installed_app_id.clone(),
            driver,
        };

        let interface_id = if model.port == 0 {
            if model.id.is_empty() {
                return Err(ConductorError::other("Port 0 interface missing ID"));
            }
            AppInterfaceId::from_parts(0, Some(model.id.clone()))
        } else {
            AppInterfaceId::new(model.port as u16)
        };
        app_interfaces.insert(interface_id, config);
    }

    Ok(ConductorState::from_parts(
        tag,
        installed_apps,
        app_interfaces,
    ))
}

/// Build a persisted [`ConductorStateSnapshot`] from a [`ConductorState`].
pub fn state_to_snapshot(state: &ConductorState) -> ConductorResult<ConductorStateSnapshot> {
    let tag = state.tag().0.as_ref().to_string();

    let installed_apps = state
        .installed_apps()
        .iter()
        .map(|(app_id, installed_app)| {
            (
                app_id.to_string(),
                installed_app.as_ref().clone(),
                installed_app.status.clone(),
            )
        })
        .collect();

    let mut app_interfaces = Vec::with_capacity(state.app_interfaces.len());
    for (interface_id, config) in &state.app_interfaces {
        let mut model = holochain_data::conductor::AppInterfaceModel::from_driver(
            &config.driver,
            config.installed_app_id.as_ref().map(|id| id.to_string()),
        )
        .map_err(ConductorError::other)?;

        let mut subscriptions = Vec::with_capacity(config.signal_subscriptions.len());
        for (app_id, subscription) in &config.signal_subscriptions {
            let filters_blob = serde_json::to_vec(subscription).map_err(|e| {
                ConductorError::other(format!("Failed to serialize signal subscription: {}", e))
            })?;
            subscriptions.push((app_id.to_string(), filters_blob));
        }

        // Keep the port/id on the model in sync with the interface_id.
        model.port = interface_id.port() as i64;
        model.id = interface_id.id().as_deref().unwrap_or("").to_string();

        app_interfaces.push((model, subscriptions));
    }

    Ok(ConductorStateSnapshot {
        tag,
        installed_apps,
        app_interfaces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conductor::state::AppInterfaceConfig;
    use holochain_state::conductor::ConductorStore;
    use holochain_types::websocket::AllowedOrigins;

    async fn state_to_store(
        store: &ConductorStore,
        state: ConductorState,
    ) -> ConductorResult<ConductorState> {
        let snapshot = state_to_snapshot(&state)?;
        store
            .update_state(|_| -> ConductorResult<_> { Ok((snapshot, ())) })
            .await?;

        let loaded_snapshot = store.as_read().load_state().await?.unwrap();
        let loaded = snapshot_to_state(loaded_snapshot)?;

        Ok(loaded)
    }

    #[tokio::test]
    async fn state_persistence_round_trip() {
        let store = ConductorStore::new_test().await.unwrap();

        let tag = ConductorStateTag(Arc::from("test-conductor"));
        let state = ConductorState::from_parts(tag, InstalledAppMap::new(), HashMap::new());

        let loaded = state_to_store(&store, state).await.unwrap();

        assert_eq!(loaded.tag().0.as_ref(), "test-conductor");
        assert_eq!(loaded.installed_apps().len(), 0);
        assert_eq!(loaded.app_interfaces.len(), 0);
    }

    #[tokio::test]
    async fn app_interface_persistence() {
        let store = ConductorStore::new_test().await.unwrap();

        let tag = ConductorStateTag(Arc::from("test-conductor"));

        let mut app_interfaces = HashMap::new();
        let interface_config =
            AppInterfaceConfig::websocket(12345, None, AllowedOrigins::Any, None);
        let interface_id = AppInterfaceId::new(12345);
        app_interfaces.insert(interface_id, interface_config);

        let state = ConductorState::from_parts(tag, InstalledAppMap::new(), app_interfaces);

        let loaded = state_to_store(&store, state).await.unwrap();

        assert_eq!(loaded.app_interfaces.len(), 1);
        let loaded_interface = loaded.app_interfaces.values().next().unwrap();
        assert_eq!(loaded_interface.driver.port(), 12345);
    }

    #[tokio::test]
    async fn deletion_of_stale_interfaces() {
        let store = ConductorStore::new_test().await.unwrap();
        let tag = ConductorStateTag(Arc::from("test-conductor"));

        // Initial state with two interfaces.
        let mut app_interfaces = HashMap::new();
        app_interfaces.insert(
            AppInterfaceId::new(12345),
            AppInterfaceConfig::websocket(12345, None, AllowedOrigins::Any, None),
        );
        app_interfaces.insert(
            AppInterfaceId::new(12346),
            AppInterfaceConfig::websocket(12346, None, AllowedOrigins::Any, None),
        );
        let state = ConductorState::from_parts(tag.clone(), InstalledAppMap::new(), app_interfaces);

        let _ = state_to_store(&store, state).await.unwrap();

        // Replace with only one interface.
        let mut app_interfaces = HashMap::new();
        app_interfaces.insert(
            AppInterfaceId::new(12345),
            AppInterfaceConfig::websocket(12345, None, AllowedOrigins::Any, None),
        );
        let new_state = ConductorState::from_parts(tag, InstalledAppMap::new(), app_interfaces);

        let loaded = state_to_store(&store, new_state).await.unwrap();

        // Loaded state has only the surviving interface.
        assert_eq!(loaded.app_interfaces.len(), 1);
        assert!(loaded
            .app_interfaces
            .contains_key(&AppInterfaceId::new(12345)));
    }
}
