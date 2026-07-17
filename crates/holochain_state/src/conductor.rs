//! A wrapper around the conductor database for managing conductor state.
//!
//! This is the preferred way for higher layers to read and write conductor
//! state: outside of tests, performing state read/write directly against
//! [`holochain_data`] is discouraged. Using the types defined there (e.g.
//! [`AppInterfaceModel`], [`WitnessNonceResult`]) is fine — they are the
//! storage representation of this data.

use holo_hash::AgentPubKey;
use holochain_conductor_api::signal_subscription::SignalSubscription;
use holochain_conductor_api::state::{
    AppInterfaceConfig, AppInterfaceId, ConductorState, ConductorStateTag,
};
use holochain_data::conductor::{AppInterfaceModel, Block, BlockTargetId, Nonce256Bits};
use holochain_data::kind::Conductor;
use holochain_data::{TxRead, TxWrite};
use holochain_types::prelude::{
    AppStatus, InitProperties, InitPropertiesMap, InstalledApp, InstalledAppCommon, InstalledAppId,
    InstalledAppMap, Timestamp,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::mutations::StateMutationResult;
use crate::prelude::StateMutationError;
use crate::query::{StateQueryError, StateQueryResult};
pub use holochain_data::conductor::{WitnessNonceResult, WITNESSABLE_EXPIRY_DURATION};

/// A single signal subscription row: `(app_id, filters_blob)`.
pub type SignalSubscriptionRow = (String, Vec<u8>);

/// An app interface row and its associated signal subscription rows.
type AppInterfaceRows = (AppInterfaceModel, Vec<SignalSubscriptionRow>);

/// The raw persisted rows backing a [`ConductorState`], read or written
/// within a single transaction.
#[derive(Clone, Debug, Default)]
struct StateRows {
    /// The conductor's self-assigned tag.
    tag: String,
    /// All installed apps and their statuses, keyed by app id.
    installed_apps: Vec<(String, InstalledAppCommon, AppStatus)>,
    /// All app interfaces paired with their signal subscriptions.
    app_interfaces: Vec<AppInterfaceRows>,
}

/// A wrapper around the conductor database.
#[derive(Clone)]
pub struct ConductorStore<Db = holochain_data::DbWrite<Conductor>> {
    db: Db,
    // Serializes `update_state` calls on this store (and its clones) so
    // concurrent read-modify-write cycles cannot interleave. Callers should
    // construct the `ConductorStore` once per conductor database and clone
    // it to share — independent `ConductorStore::new(db)` calls against the
    // same underlying handle produce separate locks and are not coordinated.
    update_lock: Arc<tokio::sync::Mutex<()>>,
}

impl<Db> std::fmt::Debug for ConductorStore<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConductorStore").finish()
    }
}

/// A read-only view of the conductor store.
pub type ConductorStoreRead = ConductorStore<holochain_data::DbRead<Conductor>>;

impl<Db> ConductorStore<Db> {
    /// Create a new `ConductorStore` from a database handle.
    pub fn new(db: Db) -> Self {
        Self {
            db,
            update_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

impl ConductorStore<holochain_data::DbRead<Conductor>> {
    /// Get the conductor tag.
    pub async fn get_conductor_tag(&self) -> StateQueryResult<Option<String>> {
        Ok(self.db.get_conductor_tag().await?)
    }

    /// Get all app interfaces.
    pub async fn get_all_app_interfaces(&self) -> StateQueryResult<Vec<AppInterfaceModel>> {
        Ok(self.db.get_all_app_interfaces().await?)
    }

    /// Get signal subscriptions for a specific app interface.
    pub async fn get_signal_subscriptions(
        &self,
        port: i64,
        id: &str,
    ) -> StateQueryResult<Vec<SignalSubscriptionRow>> {
        Ok(self.db.get_signal_subscriptions(port, id).await?)
    }

    /// Get an installed app by ID.
    pub async fn get_installed_app(
        &self,
        app_id: &str,
    ) -> StateQueryResult<Option<(InstalledAppCommon, AppStatus)>> {
        Ok(self.db.get_installed_app(app_id).await?)
    }

    /// Get the init properties for a role of an installed app.
    pub async fn get_init_properties(
        &self,
        app_id: &str,
        role_name: &str,
    ) -> StateQueryResult<Option<InitProperties>> {
        Ok(self.db.get_init_properties(app_id, role_name).await?)
    }

    /// Get all installed apps.
    pub async fn get_all_installed_apps(
        &self,
    ) -> StateQueryResult<Vec<(String, InstalledAppCommon, AppStatus)>> {
        Ok(self.db.get_all_installed_apps().await?)
    }

    /// Load the full persisted conductor state atomically.
    ///
    /// Returns `None` if the conductor tag has not yet been set
    /// (the conductor has no persisted state). Otherwise returns a
    /// [`ConductorState`] read within a single transaction so the tag,
    /// apps, interfaces, and subscriptions are all consistent with each
    /// other.
    pub async fn load_state(&self) -> StateQueryResult<Option<ConductorState>> {
        let mut tx = self.db.begin().await?;
        let rows = load_rows_in_tx(&mut tx).await?;
        tx.close().await?;
        rows.map(state_from_rows).transpose()
    }

    /// Check if a nonce has already been seen.
    pub async fn nonce_already_seen(
        &self,
        agent: &AgentPubKey,
        nonce: &Nonce256Bits,
        now: Timestamp,
    ) -> StateQueryResult<bool> {
        Ok(self.db.nonce_already_seen(agent, nonce, now).await?)
    }

    /// Check whether a given target is blocked at the given time.
    pub async fn is_blocked(
        &self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
    ) -> StateQueryResult<bool> {
        Ok(self.db.is_blocked(target_id, timestamp).await?)
    }

    /// Query whether any of the provided targets is blocked at the given timestamp.
    pub async fn is_any_blocked(
        &self,
        target_ids: Vec<BlockTargetId>,
        timestamp: Timestamp,
    ) -> StateQueryResult<bool> {
        Ok(self.db.is_any_blocked(target_ids, timestamp).await?)
    }

    /// Get all blocks from the database.
    pub async fn get_all_blocks(&self) -> StateQueryResult<Vec<Block>> {
        Ok(self.db.get_all_blocks().await?)
    }
}

impl ConductorStore<holochain_data::DbWrite<Conductor>> {
    /// Atomically update the persisted conductor state, optionally writing
    /// `init_properties` rows for `app_id` in the same transaction.
    /// When `init_properties` is empty the `app_id` is not used.
    ///
    /// Loads the current state (if any), applies `f`, and saves the
    /// result, all within a single database transaction. An internal
    /// mutex serializes concurrent callers so read-modify-write cycles
    /// cannot interleave and silently drop updates.
    ///
    /// The closure receives `None` when the conductor has no persisted
    /// state yet (the tag is unset); it must produce a new state to
    /// persist.
    pub async fn update_state<F, O, E>(
        &self,
        f: F,
        app_id: &str,
        init_properties: &InitPropertiesMap,
    ) -> Result<O, E>
    where
        F: FnOnce(Option<ConductorState>) -> Result<(ConductorState, O), E>,
        E: From<StateMutationError>,
    {
        let _guard = self.update_lock.lock().await;
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        let current = load_rows_in_tx(tx.as_mut())
            .await
            .map_err(StateMutationError::from)?
            .map(state_from_rows)
            .transpose()
            .map_err(StateMutationError::from)?;
        let (new_state, output) = f(current)?;
        let new_rows = state_to_rows(&new_state)?;
        save_rows_in_tx(&mut tx, &new_rows)
            .await
            .map_err(StateMutationError::from)?;
        for (role_name, properties) in init_properties {
            tx.put_init_properties(app_id, role_name.as_str(), properties)
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(output)
    }

    /// Witness a nonce (check if it's fresh and record it).
    pub async fn witness_nonce(
        &self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        now: Timestamp,
        expires: Timestamp,
    ) -> StateMutationResult<WitnessNonceResult> {
        Ok(self.db.witness_nonce(agent, nonce, now, expires).await?)
    }

    /// Insert a block into the database.
    pub async fn block(&self, input: Block) -> StateMutationResult<()> {
        Ok(self.db.block(input).await?)
    }

    /// Insert or update the init properties for a role of an installed app.
    pub async fn put_init_properties(
        &self,
        app_id: &str,
        role_name: &str,
        properties: &InitProperties,
    ) -> StateMutationResult<()> {
        Ok(self
            .db
            .put_init_properties(app_id, role_name, properties)
            .await?)
    }

    /// Delete the init properties for a role of an installed app.
    pub async fn delete_init_properties(
        &self,
        app_id: &str,
        role_name: &str,
    ) -> StateMutationResult<()> {
        Ok(self.db.delete_init_properties(app_id, role_name).await?)
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> ConductorStoreRead {
        ConductorStore::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> ConductorStoreRead {
        ConductorStore::new(self.db.into())
    }
}

impl From<ConductorStore<holochain_data::DbWrite<Conductor>>> for ConductorStoreRead {
    fn from(store: ConductorStore<holochain_data::DbWrite<Conductor>>) -> Self {
        store.into_read()
    }
}

impl<Db> ConductorStore<Db>
where
    Db: AsRef<holochain_data::DbRead<Conductor>>,
{
    /// Get a reference to the underlying read handle.
    pub fn raw_db_read(&self) -> &holochain_data::DbRead<Conductor> {
        self.db.as_ref()
    }
}

#[cfg(any(test, feature = "test_utils"))]
impl ConductorStore<holochain_data::DbWrite<Conductor>> {
    /// Create an in-memory conductor store for testing.
    pub async fn new_test() -> StateQueryResult<Self> {
        let db = holochain_data::test_open_db(Conductor).await?;
        Ok(Self::new(db))
    }
}

// ============================================================================
// Row load/save and conversion helpers (used by atomic read/write paths)
// ============================================================================

/// Build a [`ConductorState`] from its persisted rows.
fn state_from_rows(rows: StateRows) -> StateQueryResult<ConductorState> {
    let tag = ConductorStateTag(Arc::from(rows.tag.as_str()));

    let mut installed_apps = InstalledAppMap::new();
    for (app_id, app_common, status) in rows.installed_apps {
        let mut installed_app = InstalledApp::new_fresh(app_common);
        installed_app.status = status;
        installed_apps.insert(InstalledAppId::from(app_id), installed_app);
    }

    let mut app_interfaces = HashMap::new();
    for (model, subs_data) in rows.app_interfaces {
        let driver = model.to_driver().map_err(StateQueryError::Other)?;

        let mut signal_subscriptions = HashMap::new();
        for (app_id, filters_blob) in subs_data {
            let subscription: SignalSubscription =
                serde_json::from_slice(&filters_blob).map_err(|e| {
                    StateQueryError::Other(format!(
                        "Failed to deserialize signal subscription: {e}"
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
                return Err(StateQueryError::Other(
                    "Port 0 interface missing ID".to_string(),
                ));
            }
            AppInterfaceId::from_parts(0, Some(model.id.clone()))
        } else {
            let port = u16::try_from(model.port).map_err(|err| {
                StateQueryError::Other(format!(
                    "Invalid port number {port}: {err}",
                    port = model.port
                ))
            })?;
            AppInterfaceId::new(port)
        };
        app_interfaces.insert(interface_id, config);
    }

    Ok(ConductorState::from_parts(
        tag,
        installed_apps,
        app_interfaces,
    ))
}

/// Build the persisted rows for a [`ConductorState`].
fn state_to_rows(state: &ConductorState) -> Result<StateRows, StateMutationError> {
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
        let mut model = AppInterfaceModel::from_driver(
            &config.driver,
            config.installed_app_id.as_ref().map(|id| id.to_string()),
        )
        .map_err(StateMutationError::Other)?;

        let mut subscriptions = Vec::with_capacity(config.signal_subscriptions.len());
        for (app_id, subscription) in &config.signal_subscriptions {
            let filters_blob = serde_json::to_vec(subscription).map_err(|e| {
                StateMutationError::Other(format!("Failed to serialize signal subscription: {e}"))
            })?;
            subscriptions.push((app_id.to_string(), filters_blob));
        }

        // Keep the port/id on the model in sync with the interface_id.
        model.port = interface_id.port() as i64;
        model.id = interface_id.id().as_deref().unwrap_or("").to_string();

        app_interfaces.push((model, subscriptions));
    }

    Ok(StateRows {
        tag,
        installed_apps,
        app_interfaces,
    })
}

async fn load_rows_in_tx(tx: &mut TxRead<Conductor>) -> sqlx::Result<Option<StateRows>> {
    let tag = match tx.get_conductor_tag().await? {
        Some(tag) => tag,
        None => return Ok(None),
    };

    let installed_apps = tx.get_all_installed_apps().await?;

    let interface_models = tx.get_all_app_interfaces().await?;
    let mut app_interfaces = Vec::with_capacity(interface_models.len());
    for model in interface_models {
        let subs = tx.get_signal_subscriptions(model.port, &model.id).await?;
        app_interfaces.push((model, subs));
    }

    Ok(Some(StateRows {
        tag,
        installed_apps,
        app_interfaces,
    }))
}

async fn save_rows_in_tx(tx: &mut TxWrite<Conductor>, rows: &StateRows) -> sqlx::Result<()> {
    tx.set_conductor_tag(&rows.tag).await?;

    // Upsert apps from the new state.
    let mut new_app_ids: HashSet<&str> = HashSet::with_capacity(rows.installed_apps.len());
    for (app_id, common, status) in &rows.installed_apps {
        tx.put_installed_app(app_id, common, status).await?;
        new_app_ids.insert(app_id.as_str());
    }

    // Delete apps that are no longer in the state.
    let existing_apps = tx.as_mut().get_all_installed_apps().await?;
    let stale_app_ids: Vec<String> = existing_apps
        .into_iter()
        .map(|(app_id, _, _)| app_id)
        .filter(|app_id| !new_app_ids.contains(app_id.as_str()))
        .collect();
    for app_id in stale_app_ids {
        tx.delete_installed_app(&app_id).await?;
    }

    // Upsert interfaces and replace their subscriptions.
    let mut new_interface_keys: HashSet<(i64, String)> =
        HashSet::with_capacity(rows.app_interfaces.len());
    for (model, subscriptions) in &rows.app_interfaces {
        tx.put_app_interface(model.port, &model.id, model).await?;
        tx.delete_signal_subscriptions(model.port, &model.id)
            .await?;
        for (app_id, filters_blob) in subscriptions {
            tx.put_signal_subscription(model.port, &model.id, app_id, filters_blob)
                .await?;
        }
        new_interface_keys.insert((model.port, model.id.clone()));
    }

    // Delete interfaces that are no longer in the state.
    let existing_interfaces = tx.as_mut().get_all_app_interfaces().await?;
    let stale_interfaces: Vec<(i64, String)> = existing_interfaces
        .into_iter()
        .filter_map(|model| {
            let key = (model.port, model.id);
            (!new_interface_keys.contains(&key)).then_some(key)
        })
        .collect();
    for (port, id) in stale_interfaces {
        tx.delete_app_interface(port, &id).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
    use holochain_types::app::DisabledAppReason;
    use holochain_types::prelude::{AppManifest, AppManifestV0, AppRoleAssignment, RoleName};
    use holochain_types::websocket::AllowedOrigins;

    fn test_init_props(bytes: Vec<u8>) -> InitProperties {
        InitProperties(SerializedBytes::from(UnsafeBytes::from(bytes)))
    }

    fn make_test_app(app_id: &str) -> InstalledAppCommon {
        InstalledAppCommon::new(
            app_id,
            AgentPubKey::from_raw_36(vec![0u8; 36]),
            std::iter::empty::<(RoleName, AppRoleAssignment)>(),
            AppManifest::V0(AppManifestV0 {
                name: app_id.to_string(),
                description: None,
                roles: vec![],
                allow_deferred_memproofs: false,
                bootstrap_url: None,
                relay_url: None,
            }),
            Timestamp::now(),
        )
        .unwrap()
    }

    fn state_with_tag(tag: &str) -> ConductorState {
        ConductorState::from_parts(
            ConductorStateTag(Arc::from(tag)),
            InstalledAppMap::new(),
            HashMap::new(),
        )
    }

    fn websocket_interfaces(ports: &[u16]) -> HashMap<AppInterfaceId, AppInterfaceConfig> {
        ports
            .iter()
            .map(|&port| {
                (
                    AppInterfaceId::new(port),
                    AppInterfaceConfig::websocket(port, None, AllowedOrigins::Any, None),
                )
            })
            .collect()
    }

    #[test]
    fn state_from_rows_rejects_ports_outside_u16_range() {
        let invalid_ports = [-1, i64::from(u16::MAX) + 1];

        for port in invalid_ports {
            let config = AppInterfaceConfig::websocket(1, None, AllowedOrigins::Any, None);
            let mut model = AppInterfaceModel::from_driver(&config.driver, None).unwrap();
            model.port = port;
            let rows = StateRows {
                tag: "test".to_string(),
                app_interfaces: vec![(model, Vec::new())],
                ..Default::default()
            };

            assert!(
                matches!(
                    state_from_rows(rows),
                    Err(StateQueryError::Other(error))
                        if error.contains(&format!("Invalid port number {port}"))
                ),
                "port {port} should be rejected"
            );
        }
    }

    async fn roundtrip(
        store: &ConductorStore,
        state: ConductorState,
    ) -> StateMutationResult<ConductorState> {
        store
            .update_state(
                |_| -> StateMutationResult<_> { Ok((state, ())) },
                "",
                &InitPropertiesMap::new(),
            )
            .await?;
        Ok(store.as_read().load_state().await?.unwrap())
    }

    #[tokio::test]
    async fn load_state_returns_none_when_unset() {
        let store = ConductorStore::new_test().await.unwrap();
        assert!(store.as_read().load_state().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_state_initializes_and_roundtrips() {
        let store = ConductorStore::new_test().await.unwrap();
        let loaded = roundtrip(&store, state_with_tag("conductor-A"))
            .await
            .unwrap();
        assert_eq!(loaded.tag().0.as_ref(), "conductor-A");
        assert!(loaded.installed_apps().is_empty());
        assert!(loaded.app_interfaces.is_empty());
    }

    #[tokio::test]
    async fn update_state_deletes_stale_interfaces() {
        let store = ConductorStore::new_test().await.unwrap();

        let two = ConductorState::from_parts(
            ConductorStateTag(Arc::from("t")),
            InstalledAppMap::new(),
            websocket_interfaces(&[12345, 12346]),
        );
        let loaded = roundtrip(&store, two).await.unwrap();
        assert_eq!(loaded.app_interfaces.len(), 2);

        let one = ConductorState::from_parts(
            ConductorStateTag(Arc::from("t")),
            InstalledAppMap::new(),
            websocket_interfaces(&[12345]),
        );
        let loaded = roundtrip(&store, one).await.unwrap();
        assert_eq!(loaded.app_interfaces.len(), 1);
        assert!(loaded
            .app_interfaces
            .contains_key(&AppInterfaceId::new(12345)));
        let iface = loaded.app_interfaces.values().next().unwrap();
        assert_eq!(iface.driver.port(), 12345);
    }

    #[tokio::test]
    async fn update_state_rolls_back_on_closure_error() {
        let store = ConductorStore::new_test().await.unwrap();

        // Seed a known state.
        roundtrip(&store, state_with_tag("seed")).await.unwrap();

        // Closure returns an error — tx must roll back.
        #[derive(Debug)]
        struct Boom;
        impl From<StateMutationError> for Boom {
            fn from(_: StateMutationError) -> Self {
                Boom
            }
        }
        let result: Result<(), Boom> = store
            .update_state(
                |_| -> Result<(ConductorState, ()), Boom> { Err(Boom) },
                "",
                &InitPropertiesMap::new(),
            )
            .await;
        assert!(result.is_err());

        // Seeded state still intact.
        let loaded = store.as_read().load_state().await.unwrap().unwrap();
        assert_eq!(loaded.tag().0.as_ref(), "seed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_update_state_does_not_lose_writes() {
        // Each task appends a unique interface to the state. Without
        // serialization, concurrent tasks would load the same state in
        // parallel and the last writer would clobber the others.
        let store = ConductorStore::new_test().await.unwrap();

        let ports: Vec<u16> = (1000..1016).collect();
        let mut joins = Vec::new();
        for port in ports.clone() {
            let store = store.clone();
            joins.push(tokio::spawn(async move {
                store
                    .update_state(
                        move |state| -> StateMutationResult<_> {
                            let mut state = state.unwrap_or_default();
                            state.app_interfaces.insert(
                                AppInterfaceId::new(port),
                                AppInterfaceConfig::websocket(
                                    port,
                                    None,
                                    AllowedOrigins::Any,
                                    None,
                                ),
                            );
                            Ok((state, ()))
                        },
                        "",
                        &InitPropertiesMap::new(),
                    )
                    .await
                    .unwrap();
            }));
        }

        for j in joins {
            j.await.unwrap();
        }

        let loaded = store.as_read().load_state().await.unwrap().unwrap();
        let mut loaded_ports: Vec<u16> = loaded.app_interfaces.keys().map(|id| id.port()).collect();
        loaded_ports.sort();
        let mut expected = ports;
        expected.sort();
        assert_eq!(loaded_ports, expected);
    }

    #[tokio::test]
    async fn update_state_writes_init_properties_in_same_transaction() {
        let store = ConductorStore::new_test().await.unwrap();

        let app_id = "my-app";
        let role = "my-role";
        let props: InitPropertiesMap = [(role.to_string(), test_init_props(vec![1, 2, 3]))].into();

        let mut apps = InstalledAppMap::new();
        apps.insert(
            app_id.to_string(),
            InstalledApp::new(
                make_test_app(app_id),
                AppStatus::Disabled(DisabledAppReason::NeverStarted),
            ),
        );
        let state =
            ConductorState::from_parts(ConductorStateTag(Arc::from("t")), apps, HashMap::new());

        store
            .update_state(
                move |_| -> StateMutationResult<_> { Ok((state, ())) },
                app_id,
                &props,
            )
            .await
            .unwrap();

        assert!(
            store
                .as_read()
                .get_installed_app(app_id)
                .await
                .unwrap()
                .is_some(),
            "app row should be present"
        );
        assert_eq!(
            store
                .as_read()
                .get_init_properties(app_id, role)
                .await
                .unwrap(),
            Some(test_init_props(vec![1, 2, 3])),
            "init_properties should be present"
        );
    }

    #[tokio::test]
    async fn update_state_init_properties_failure_rolls_back_state_change() {
        let store = ConductorStore::new_test().await.unwrap();

        // Seed an initial state.
        roundtrip(&store, state_with_tag("initial")).await.unwrap();

        // Attempt a state change that also tries to write init_properties for an
        // app that will not exist after the state is saved. The FK violation
        // must roll back the entire transaction, including the state change.
        let props: InitPropertiesMap =
            [("role".to_string(), test_init_props(vec![4, 5, 6]))].into();

        let result: StateMutationResult<()> = store
            .update_state(
                |_| -> StateMutationResult<_> { Ok((state_with_tag("changed"), ())) },
                "nonexistent-app",
                &props,
            )
            .await;

        assert!(result.is_err(), "FK violation should return an error");

        let loaded = store.as_read().load_state().await.unwrap().unwrap();
        assert_eq!(
            loaded.tag().0.as_ref(),
            "initial",
            "state change should have been rolled back"
        );
    }

    #[tokio::test]
    async fn conductor_tag_roundtrip() {
        let store = ConductorStore::new_test().await.unwrap();
        assert_eq!(store.as_read().get_conductor_tag().await.unwrap(), None);

        roundtrip(&store, state_with_tag("hello")).await.unwrap();

        assert_eq!(
            store.as_read().get_conductor_tag().await.unwrap(),
            Some("hello".to_string())
        );
    }
}
