//! A wrapper around the conductor database for managing conductor state.
//!
//! This is the preferred way for higher layers to access the conductor
//! database: direct use of [`holochain_data`] from outside this crate is
//! discouraged.

use holo_hash::AgentPubKey;
use holochain_data::conductor::{AppInterfaceModel, Block, BlockTargetId, Nonce256Bits};
use holochain_types::prelude::{AppStatus, InstalledAppCommon, Timestamp};

pub use holochain_data::conductor::{WitnessNonceResult, WITNESSABLE_EXPIRY_DURATION};

/// A wrapper around the conductor database.
#[derive(Clone)]
pub struct ConductorStore<Db = holochain_data::DbWrite<holochain_data::kind::Conductor>> {
    db: Db,
}

impl<Db> std::fmt::Debug for ConductorStore<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConductorStore").finish()
    }
}

/// A read-only view of the conductor store.
pub type ConductorStoreRead =
    ConductorStore<holochain_data::DbRead<holochain_data::kind::Conductor>>;

impl<Db> ConductorStore<Db> {
    /// Create a new `ConductorStore` from a database handle.
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl ConductorStore<holochain_data::DbRead<holochain_data::kind::Conductor>> {
    /// Get the conductor tag.
    pub async fn get_conductor_tag(&self) -> sqlx::Result<Option<String>> {
        self.db.get_conductor_tag().await
    }

    /// Get all app interfaces.
    pub async fn get_all_app_interfaces(&self) -> sqlx::Result<Vec<AppInterfaceModel>> {
        self.db.get_all_app_interfaces().await
    }

    /// Get signal subscriptions for a specific app interface.
    pub async fn get_signal_subscriptions(
        &self,
        port: i64,
        id: &str,
    ) -> sqlx::Result<Vec<(String, Vec<u8>)>> {
        self.db.get_signal_subscriptions(port, id).await
    }

    /// Get an installed app by ID.
    pub async fn get_installed_app(
        &self,
        app_id: &str,
    ) -> sqlx::Result<Option<(InstalledAppCommon, AppStatus)>> {
        self.db.get_installed_app(app_id).await
    }

    /// Get all installed apps.
    pub async fn get_all_installed_apps(
        &self,
    ) -> sqlx::Result<Vec<(String, InstalledAppCommon, AppStatus)>> {
        self.db.get_all_installed_apps().await
    }

    /// Check if a nonce has already been seen.
    pub async fn nonce_already_seen(
        &self,
        agent: &AgentPubKey,
        nonce: &Nonce256Bits,
        now: Timestamp,
    ) -> sqlx::Result<bool> {
        self.db.nonce_already_seen(agent, nonce, now).await
    }

    /// Check whether a given target is blocked at the given time.
    pub async fn is_blocked(
        &self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
    ) -> sqlx::Result<bool> {
        self.db.is_blocked(target_id, timestamp).await
    }

    /// Query whether any of the provided targets is blocked at the given timestamp.
    pub async fn is_any_blocked(
        &self,
        target_ids: Vec<BlockTargetId>,
        timestamp: Timestamp,
    ) -> sqlx::Result<bool> {
        self.db.is_any_blocked(target_ids, timestamp).await
    }

    /// Get all blocks from the database.
    pub async fn get_all_blocks(&self) -> sqlx::Result<Vec<Block>> {
        self.db.get_all_blocks().await
    }
}

impl ConductorStore<holochain_data::DbWrite<holochain_data::kind::Conductor>> {
    /// Set the conductor tag.
    pub async fn set_conductor_tag(&self, tag: &str) -> sqlx::Result<()> {
        self.db.set_conductor_tag(tag).await
    }

    /// Insert or update an app interface.
    pub async fn put_app_interface(
        &self,
        port: i64,
        id: &str,
        model: &AppInterfaceModel,
    ) -> sqlx::Result<()> {
        self.db.put_app_interface(port, id, model).await
    }

    /// Save a signal subscription for an app interface.
    pub async fn put_signal_subscription(
        &self,
        interface_port: i64,
        interface_id: &str,
        app_id: &str,
        filters_blob: &[u8],
    ) -> sqlx::Result<()> {
        self.db
            .put_signal_subscription(interface_port, interface_id, app_id, filters_blob)
            .await
    }

    /// Delete all signal subscriptions for an app interface.
    pub async fn delete_signal_subscriptions(
        &self,
        interface_port: i64,
        interface_id: &str,
    ) -> sqlx::Result<()> {
        self.db
            .delete_signal_subscriptions(interface_port, interface_id)
            .await
    }

    /// Delete an app interface.
    pub async fn delete_app_interface(&self, port: i64, id: &str) -> sqlx::Result<()> {
        self.db.delete_app_interface(port, id).await
    }

    /// Insert or update an installed app.
    pub async fn put_installed_app(
        &self,
        app_id: &str,
        app: &InstalledAppCommon,
        status: &AppStatus,
    ) -> sqlx::Result<()> {
        self.db.put_installed_app(app_id, app, status).await
    }

    /// Delete an installed app.
    pub async fn delete_installed_app(&self, app_id: &str) -> sqlx::Result<()> {
        self.db.delete_installed_app(app_id).await
    }

    /// Witness a nonce (check if it's fresh and record it).
    pub async fn witness_nonce(
        &self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        now: Timestamp,
        expires: Timestamp,
    ) -> sqlx::Result<WitnessNonceResult> {
        self.db.witness_nonce(agent, nonce, now, expires).await
    }

    /// Insert a block into the database, merging with overlapping blocks.
    pub async fn block(&self, input: Block) -> sqlx::Result<()> {
        self.db.block(input).await
    }

    /// Insert an unblock into the database, splitting existing blocks as needed.
    pub async fn unblock(&self, input: Block) -> sqlx::Result<()> {
        self.db.unblock(input).await
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> ConductorStoreRead {
        ConductorStore::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> ConductorStoreRead {
        ConductorStore::new(self.db.as_ref().clone())
    }
}

impl From<ConductorStore<holochain_data::DbWrite<holochain_data::kind::Conductor>>>
    for ConductorStoreRead
{
    fn from(
        store: ConductorStore<holochain_data::DbWrite<holochain_data::kind::Conductor>>,
    ) -> Self {
        store.into_read()
    }
}

impl<Db> ConductorStore<Db>
where
    Db: AsRef<holochain_data::DbRead<holochain_data::kind::Conductor>>,
{
    /// Get a reference to the underlying read handle.
    pub fn raw_db_read(&self) -> &holochain_data::DbRead<holochain_data::kind::Conductor> {
        self.db.as_ref()
    }
}

#[cfg(any(test, feature = "test_utils"))]
impl ConductorStore<holochain_data::DbWrite<holochain_data::kind::Conductor>> {
    /// Create an in-memory conductor store for testing.
    pub async fn new_test() -> sqlx::Result<Self> {
        let db = holochain_data::test_open_db(holochain_data::kind::Conductor).await?;
        Ok(Self::new(db))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn conductor_tag_roundtrip() {
        let store = ConductorStore::new_test().await.unwrap();
        assert_eq!(store.as_read().get_conductor_tag().await.unwrap(), None);

        store.set_conductor_tag("hello").await.unwrap();
        assert_eq!(
            store.as_read().get_conductor_tag().await.unwrap(),
            Some("hello".to_string())
        );
    }
}
