//! A wrapper around the conductor database for managing conductor state.
//!
//! This is the preferred way for higher layers to read and write conductor
//! state: outside of tests, performing state read/write directly against
//! [`holochain_data`] is discouraged. Using the types defined there (e.g.
//! [`AppInterfaceModel`], [`WitnessNonceResult`]) is fine — they are the
//! wire format for this data.

use holo_hash::AgentPubKey;
use holochain_data::conductor::{AppInterfaceModel, Block, BlockTargetId, Nonce256Bits};
use holochain_data::kind::Conductor;
use holochain_data::{TxRead, TxWrite};
use holochain_types::prelude::{AppStatus, InstalledAppCommon, Timestamp};
use std::collections::HashSet;
use std::sync::Arc;

pub use holochain_data::conductor::{WitnessNonceResult, WITNESSABLE_EXPIRY_DURATION};

/// Errors produced by [`ConductorStore`] operations.
///
/// Wraps the underlying data-layer error so callers do not need to depend on
/// `sqlx` directly.
#[derive(thiserror::Error, Debug)]
pub enum ConductorStoreError {
    /// An underlying database operation failed.
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Convenience alias for [`ConductorStore`] results.
pub type ConductorStoreResult<T> = Result<T, ConductorStoreError>;

/// A point-in-time view of the persisted conductor state.
///
/// Used as the read/write unit for atomic state updates via
/// [`ConductorStore::update_state`]. Producing or consuming this type
/// does not touch the database on its own.
#[derive(Clone, Debug, Default)]
pub struct ConductorStateSnapshot {
    /// The conductor's self-assigned tag.
    pub tag: String,
    /// All installed apps and their statuses, keyed by app id.
    pub installed_apps: Vec<(String, InstalledAppCommon, AppStatus)>,
    /// All app interfaces paired with their signal subscriptions.
    ///
    /// Each entry contains the interface model and a list of
    /// `(app_id, filters_blob)` subscription rows.
    pub app_interfaces: Vec<(AppInterfaceModel, Vec<(String, Vec<u8>)>)>,
}

/// A wrapper around the conductor database.
#[derive(Clone)]
pub struct ConductorStore<Db = holochain_data::DbWrite<Conductor>> {
    db: Db,
    // Serializes `update_state` calls so concurrent read-modify-write
    // cycles cannot interleave. Shared across clones so any two handles
    // that refer to the same conductor database also share the lock.
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
    pub async fn get_conductor_tag(&self) -> ConductorStoreResult<Option<String>> {
        Ok(self.db.get_conductor_tag().await?)
    }

    /// Get all app interfaces.
    pub async fn get_all_app_interfaces(&self) -> ConductorStoreResult<Vec<AppInterfaceModel>> {
        Ok(self.db.get_all_app_interfaces().await?)
    }

    /// Get signal subscriptions for a specific app interface.
    pub async fn get_signal_subscriptions(
        &self,
        port: i64,
        id: &str,
    ) -> ConductorStoreResult<Vec<(String, Vec<u8>)>> {
        Ok(self.db.get_signal_subscriptions(port, id).await?)
    }

    /// Get an installed app by ID.
    pub async fn get_installed_app(
        &self,
        app_id: &str,
    ) -> ConductorStoreResult<Option<(InstalledAppCommon, AppStatus)>> {
        Ok(self.db.get_installed_app(app_id).await?)
    }

    /// Get all installed apps.
    pub async fn get_all_installed_apps(
        &self,
    ) -> ConductorStoreResult<Vec<(String, InstalledAppCommon, AppStatus)>> {
        Ok(self.db.get_all_installed_apps().await?)
    }

    /// Load the full persisted conductor state atomically.
    ///
    /// Returns `None` if the conductor tag has not yet been set
    /// (the conductor has no persisted state). Otherwise returns a
    /// [`ConductorStateSnapshot`] read within a single transaction so
    /// the tag, apps, interfaces, and subscriptions are all consistent
    /// with each other.
    pub async fn load_state(&self) -> ConductorStoreResult<Option<ConductorStateSnapshot>> {
        let mut tx = self.db.begin().await?;
        let snapshot = load_snapshot_in_tx(&mut tx).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    /// Check if a nonce has already been seen.
    pub async fn nonce_already_seen(
        &self,
        agent: &AgentPubKey,
        nonce: &Nonce256Bits,
        now: Timestamp,
    ) -> ConductorStoreResult<bool> {
        Ok(self.db.nonce_already_seen(agent, nonce, now).await?)
    }

    /// Check whether a given target is blocked at the given time.
    pub async fn is_blocked(
        &self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
    ) -> ConductorStoreResult<bool> {
        Ok(self.db.is_blocked(target_id, timestamp).await?)
    }

    /// Query whether any of the provided targets is blocked at the given timestamp.
    pub async fn is_any_blocked(
        &self,
        target_ids: Vec<BlockTargetId>,
        timestamp: Timestamp,
    ) -> ConductorStoreResult<bool> {
        Ok(self.db.is_any_blocked(target_ids, timestamp).await?)
    }

    /// Get all blocks from the database.
    pub async fn get_all_blocks(&self) -> ConductorStoreResult<Vec<Block>> {
        Ok(self.db.get_all_blocks().await?)
    }
}

impl ConductorStore<holochain_data::DbWrite<Conductor>> {
    /// Atomically update the persisted conductor state.
    ///
    /// Loads the current snapshot (if any), applies `f`, and saves the
    /// result, all within a single database transaction. An internal
    /// mutex serializes concurrent callers so read-modify-write cycles
    /// cannot interleave and silently drop updates.
    ///
    /// The closure receives `None` when the conductor has no persisted
    /// state yet (the tag is unset); it must produce a new snapshot to
    /// persist.
    pub async fn update_state<F, O, E>(&self, f: F) -> Result<O, E>
    where
        F: FnOnce(Option<ConductorStateSnapshot>) -> Result<(ConductorStateSnapshot, O), E>,
        E: From<ConductorStoreError>,
    {
        let _guard = self.update_lock.lock().await;
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| E::from(ConductorStoreError::from(e)))?;
        let current = load_snapshot_in_tx(tx.as_mut())
            .await
            .map_err(|e| E::from(ConductorStoreError::from(e)))?;
        let (new_snapshot, output) = f(current)?;
        save_snapshot_in_tx(&mut tx, &new_snapshot)
            .await
            .map_err(|e| E::from(ConductorStoreError::from(e)))?;
        tx.commit()
            .await
            .map_err(|e| E::from(ConductorStoreError::from(e)))?;
        Ok(output)
    }

    /// Witness a nonce (check if it's fresh and record it).
    pub async fn witness_nonce(
        &self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        now: Timestamp,
        expires: Timestamp,
    ) -> ConductorStoreResult<WitnessNonceResult> {
        Ok(self.db.witness_nonce(agent, nonce, now, expires).await?)
    }

    /// Insert a block into the database, merging with overlapping blocks.
    pub async fn block(&self, input: Block) -> ConductorStoreResult<()> {
        Ok(self.db.block(input).await?)
    }

    /// Insert an unblock into the database, splitting existing blocks as needed.
    pub async fn unblock(&self, input: Block) -> ConductorStoreResult<()> {
        Ok(self.db.unblock(input).await?)
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
    pub async fn new_test() -> ConductorStoreResult<Self> {
        let db = holochain_data::test_open_db(Conductor).await?;
        Ok(Self::new(db))
    }
}

// ============================================================================
// Snapshot load/save helpers (used by atomic read/write paths)
// ============================================================================

async fn load_snapshot_in_tx(
    tx: &mut TxRead<Conductor>,
) -> sqlx::Result<Option<ConductorStateSnapshot>> {
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

    Ok(Some(ConductorStateSnapshot {
        tag,
        installed_apps,
        app_interfaces,
    }))
}

async fn save_snapshot_in_tx(
    tx: &mut TxWrite<Conductor>,
    snapshot: &ConductorStateSnapshot,
) -> sqlx::Result<()> {
    tx.set_conductor_tag(&snapshot.tag).await?;

    // Upsert apps from the new snapshot.
    let mut new_app_ids: HashSet<&str> = HashSet::with_capacity(snapshot.installed_apps.len());
    for (app_id, common, status) in &snapshot.installed_apps {
        tx.put_installed_app(app_id, common, status).await?;
        new_app_ids.insert(app_id.as_str());
    }

    // Delete apps that are no longer in the snapshot.
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
        HashSet::with_capacity(snapshot.app_interfaces.len());
    for (model, subscriptions) in &snapshot.app_interfaces {
        tx.put_app_interface(model.port, &model.id, model).await?;
        tx.delete_signal_subscriptions(model.port, &model.id)
            .await?;
        for (app_id, filters_blob) in subscriptions {
            tx.put_signal_subscription(model.port, &model.id, app_id, filters_blob)
                .await?;
        }
        new_interface_keys.insert((model.port, model.id.clone()));
    }

    // Delete interfaces that are no longer in the snapshot.
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

    #[tokio::test(flavor = "multi_thread")]
    async fn load_state_returns_none_when_unset() {
        let store = ConductorStore::new_test().await.unwrap();
        assert!(store.as_read().load_state().await.unwrap().is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_state_initializes_and_roundtrips() {
        let store = ConductorStore::new_test().await.unwrap();

        // Initialize with a tag
        store
            .update_state(|snap| -> Result<_, ConductorStoreError> {
                assert!(snap.is_none());
                Ok((
                    ConductorStateSnapshot {
                        tag: "conductor-A".to_string(),
                        installed_apps: vec![],
                        app_interfaces: vec![],
                    },
                    (),
                ))
            })
            .await
            .unwrap();

        let loaded = store.as_read().load_state().await.unwrap().unwrap();
        assert_eq!(loaded.tag, "conductor-A");
        assert!(loaded.installed_apps.is_empty());
        assert!(loaded.app_interfaces.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_state_deletes_stale_interfaces() {
        let store = ConductorStore::new_test().await.unwrap();

        let iface = |port: i64| AppInterfaceModel {
            port,
            id: String::new(),
            driver_type: "websocket".to_string(),
            websocket_port: Some(port),
            danger_bind_addr: None,
            allowed_origins_blob: None,
            installed_app_id: None,
        };

        // Save two interfaces.
        store
            .update_state(|_| -> Result<_, ConductorStoreError> {
                Ok((
                    ConductorStateSnapshot {
                        tag: "t".to_string(),
                        installed_apps: vec![],
                        app_interfaces: vec![(iface(1111), vec![]), (iface(2222), vec![])],
                    },
                    (),
                ))
            })
            .await
            .unwrap();

        // Replace with just one interface.
        store
            .update_state(|snap| -> Result<_, ConductorStoreError> {
                let snap = snap.unwrap();
                assert_eq!(snap.app_interfaces.len(), 2);
                Ok((
                    ConductorStateSnapshot {
                        tag: snap.tag,
                        installed_apps: snap.installed_apps,
                        app_interfaces: vec![(iface(1111), vec![])],
                    },
                    (),
                ))
            })
            .await
            .unwrap();

        let loaded = store.as_read().load_state().await.unwrap().unwrap();
        assert_eq!(loaded.app_interfaces.len(), 1);
        assert_eq!(loaded.app_interfaces[0].0.port, 1111);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_state_rolls_back_on_closure_error() {
        let store = ConductorStore::new_test().await.unwrap();

        // Seed a known state.
        store
            .update_state(|_| -> Result<_, ConductorStoreError> {
                Ok((
                    ConductorStateSnapshot {
                        tag: "seed".to_string(),
                        installed_apps: vec![],
                        app_interfaces: vec![],
                    },
                    (),
                ))
            })
            .await
            .unwrap();

        // Closure returns an error — tx must roll back.
        #[derive(Debug)]
        struct Boom;
        impl From<ConductorStoreError> for Boom {
            fn from(_: ConductorStoreError) -> Self {
                Boom
            }
        }
        let result: Result<(), Boom> = store
            .update_state(|_| -> Result<(ConductorStateSnapshot, ()), Boom> { Err(Boom) })
            .await;
        assert!(result.is_err());

        // Seeded state still intact.
        let loaded = store.as_read().load_state().await.unwrap().unwrap();
        assert_eq!(loaded.tag, "seed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_update_state_does_not_lose_writes() {
        // Each task appends a unique interface to the snapshot. Without
        // serialization, concurrent tasks would load the same snapshot in
        // parallel and the last writer would clobber the others.
        let store = ConductorStore::new_test().await.unwrap();

        let ports: Vec<i64> = (0..16).collect();
        let mut joins = Vec::new();
        for port in ports.clone() {
            let store = store.clone();
            joins.push(tokio::spawn(async move {
                store
                    .update_state(move |snap| -> Result<_, ConductorStoreError> {
                        let mut snap = snap.unwrap_or_default();
                        if snap.tag.is_empty() {
                            snap.tag = "t".to_string();
                        }
                        let model = AppInterfaceModel {
                            port,
                            id: String::new(),
                            driver_type: "websocket".to_string(),
                            websocket_port: Some(port),
                            danger_bind_addr: None,
                            allowed_origins_blob: None,
                            installed_app_id: None,
                        };
                        snap.app_interfaces.push((model, vec![]));
                        Ok((snap, ()))
                    })
                    .await
                    .unwrap();
            }));
        }

        for j in joins {
            j.await.unwrap();
        }

        let loaded = store.as_read().load_state().await.unwrap().unwrap();
        let mut loaded_ports: Vec<i64> =
            loaded.app_interfaces.into_iter().map(|(m, _)| m.port).collect();
        loaded_ports.sort();
        let mut expected = ports;
        expected.sort();
        assert_eq!(loaded_ports, expected);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conductor_tag_roundtrip() {
        let store = ConductorStore::new_test().await.unwrap();
        assert_eq!(store.as_read().get_conductor_tag().await.unwrap(), None);

        store
            .update_state(|_| -> Result<_, ConductorStoreError> {
                Ok((
                    ConductorStateSnapshot {
                        tag: "hello".to_string(),
                        installed_apps: vec![],
                        app_interfaces: vec![],
                    },
                    (),
                ))
            })
            .await
            .unwrap();

        assert_eq!(
            store.as_read().get_conductor_tag().await.unwrap(),
            Some("hello".to_string())
        );
    }
}
