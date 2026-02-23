//! Conductor database models and operations
//!
//! This module provides data models and database operations for the conductor database,
//! which stores the conductor's state including installed apps, roles, and interfaces.

use holochain_conductor_api::config::InterfaceDriver;
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::prelude::*;
use holochain_types::websocket::AllowedOrigins;
use std::collections::HashSet;

pub use holochain_nonce::Nonce256Bits;
pub use holochain_timestamp::InclusiveTimestampInterval;
pub use holochain_zome_types::block::{Block, BlockTargetId};

/// Model for the Conductor table (singleton)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConductorModel {
    pub id: i64,
    pub tag: String,
}

/// Model for InstalledApp table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InstalledAppModel {
    pub app_id: String,
    pub agent_pub_key: Vec<u8>,
    pub status: String,
    pub disabled_reason: Option<String>,
    pub manifest_blob: Vec<u8>,
    pub role_assignments_blob: Vec<u8>,
    pub installed_at: i64,
}

impl InstalledAppModel {
    /// Convert from InstalledApp and InstalledAppCommon
    pub fn from_installed_app(
        app_id: &str,
        app: &InstalledAppCommon,
        status: &AppStatus,
    ) -> Result<Self, String> {
        let agent_bytes = app.agent_key().get_raw_36().to_vec();
        let (status_str, disabled_reason) = match status {
            AppStatus::Enabled => ("enabled".to_string(), None),
            AppStatus::Disabled(reason) => {
                let reason_json = serde_json::to_string(reason)
                    .map_err(|e| format!("Failed to serialize disabled reason: {}", e))?;
                ("disabled".to_string(), Some(reason_json))
            }
            AppStatus::AwaitingMemproofs => ("awaiting_memproofs".to_string(), None),
        };

        // Serialize the manifest using serde
        let manifest_bytes = serde_json::to_vec(app.manifest())
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
        let manifest_blob = manifest_bytes;

        // Serialize role_assignments using serde
        let role_assignments_bytes = serde_json::to_vec(app.role_assignments())
            .map_err(|e| format!("Failed to serialize role_assignments: {}", e))?;

        let installed_at = app.installed_at().as_micros();

        Ok(Self {
            app_id: app_id.to_string(),
            agent_pub_key: agent_bytes,
            status: status_str,
            disabled_reason,
            manifest_blob,
            role_assignments_blob: role_assignments_bytes,
            installed_at,
        })
    }

    /// Convert back to InstalledAppCommon and AppStatus
    pub fn to_installed_app(&self) -> Result<(InstalledAppCommon, AppStatus), String> {
        use holo_hash::AgentPubKey;
        use holochain_integrity_types::prelude::Timestamp;

        // Deserialize agent key
        let agent_key = AgentPubKey::from_raw_36(self.agent_pub_key.clone());

        // Deserialize manifest
        let manifest: AppManifest = serde_json::from_slice(&self.manifest_blob)
            .map_err(|e| format!("Failed to deserialize manifest: {}", e))?;

        // Deserialize role_assignments
        let role_assignments: indexmap::IndexMap<RoleName, AppRoleAssignment> =
            serde_json::from_slice(&self.role_assignments_blob)
                .map_err(|e| format!("Failed to deserialize role_assignments: {}", e))?;

        // Parse status
        let status = match self.status.as_str() {
            "enabled" => AppStatus::Enabled,
            "disabled" => {
                let reason_str = self
                    .disabled_reason
                    .as_ref()
                    .ok_or_else(|| "Missing disabled reason".to_string())?;
                let reason: DisabledAppReason = serde_json::from_str(reason_str)
                    .map_err(|e| format!("Failed to deserialize disabled reason: {}", e))?;
                AppStatus::Disabled(reason)
            }
            "awaiting_memproofs" => AppStatus::AwaitingMemproofs,
            _ => return Err(format!("Unknown status: {}", self.status)),
        };

        // Convert timestamp
        let installed_at = Timestamp::from_micros(self.installed_at);

        let app = InstalledAppCommon::new(
            self.app_id.clone(),
            agent_key,
            role_assignments,
            manifest,
            installed_at,
        )
        .map_err(|e| format!("Failed to create InstalledAppCommon: {:?}", e))?;

        Ok((app, status))
    }
}

/// Model for AppRole table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AppRoleModel {
    pub app_id: String,
    pub role_name: String,
    pub dna_hash: Vec<u8>,
    pub is_clone_limit_enabled: i64,
    pub clone_limit: i64,
}

/// Model for CloneCell table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CloneCellModel {
    pub app_id: String,
    pub role_name: String,
    pub clone_id: String,
    pub dna_hash: Vec<u8>,
    pub is_enabled: i64,
}

/// Model for AppInterface table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AppInterfaceModel {
    pub port: i64,
    pub id: Option<String>,
    pub driver_type: String,
    pub websocket_port: Option<i64>,
    pub danger_bind_addr: Option<String>,
    pub allowed_origins_blob: Option<Vec<u8>>,
    pub installed_app_id: Option<String>,
}

impl AppInterfaceModel {
    /// Create from InterfaceDriver and installed_app_id
    pub fn from_driver(
        driver: &InterfaceDriver,
        installed_app_id: Option<String>,
    ) -> Result<Self, String> {
        match driver {
            InterfaceDriver::Websocket {
                port,
                danger_bind_addr,
                allowed_origins,
            } => {
                // Serialize allowed_origins
                let allowed_origins_blob = serde_json::to_vec(allowed_origins)
                    .map_err(|e| format!("Failed to serialize allowed_origins: {}", e))?;

                Ok(Self {
                    port: *port as i64,
                    id: None,
                    driver_type: "websocket".to_string(),
                    websocket_port: Some(*port as i64),
                    danger_bind_addr: danger_bind_addr.clone(),
                    allowed_origins_blob: Some(allowed_origins_blob),
                    installed_app_id,
                })
            }
        }
    }

    /// Convert back to InterfaceDriver
    pub fn to_driver(&self) -> Result<InterfaceDriver, String> {
        if self.driver_type != "websocket" {
            return Err(format!("Unknown driver type: {}", self.driver_type));
        }

        let port = self.websocket_port.ok_or("Missing websocket_port")? as u16;
        let danger_bind_addr = self.danger_bind_addr.clone();
        let allowed_origins = if let Some(ref blob) = self.allowed_origins_blob {
            serde_json::from_slice(blob)
                .map_err(|e| format!("Failed to deserialize allowed_origins: {}", e))?
        } else {
            AllowedOrigins::Any
        };

        Ok(InterfaceDriver::Websocket {
            port,
            danger_bind_addr,
            allowed_origins,
        })
    }
}

// Database operations will be implemented here
// These will be async functions that operate on DbRead<Conductor> and DbWrite<Conductor>

use crate::handles::{DbRead, DbWrite};
use crate::kind::Conductor;

// Read operations
impl DbRead<Conductor> {
    /// Get the conductor tag.
    pub async fn get_conductor_tag(&self) -> sqlx::Result<Option<String>> {
        let tag: Option<String> = sqlx::query_scalar("SELECT tag FROM Conductor WHERE id = 1")
            .fetch_optional(self.pool())
            .await?;
        Ok(tag)
    }

    /// Get all app interfaces.
    pub async fn get_all_app_interfaces(&self) -> sqlx::Result<Vec<AppInterfaceModel>> {
        let models: Vec<AppInterfaceModel> = sqlx::query_as(
            "SELECT port, id, driver_type, websocket_port, danger_bind_addr, allowed_origins_blob, installed_app_id FROM AppInterface",
        )
        .fetch_all(self.pool())
        .await?;

        Ok(models)
    }

    /// Get signal subscriptions for a specific app interface.
    pub async fn get_signal_subscriptions(
        &self,
        port: i64,
        id: Option<&str>,
    ) -> sqlx::Result<Vec<(String, Vec<u8>)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            app_id: String,
            filters_blob: Option<Vec<u8>>,
        }

        let rows: Vec<Row> = sqlx::query_as(
            "SELECT app_id, filters_blob FROM SignalSubscription WHERE interface_port = ? AND interface_id IS ?",
        )
        .bind(port)
        .bind(id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| r.filters_blob.map(|blob| (r.app_id, blob)))
            .collect())
    }

    /// Get an installed app by ID.
    pub async fn get_installed_app(
        &self,
        app_id: &str,
    ) -> sqlx::Result<Option<(InstalledAppCommon, AppStatus)>> {
        let model: Option<InstalledAppModel> = sqlx::query_as(
            "SELECT app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at FROM InstalledApp WHERE app_id = ?",
        )
        .bind(app_id)
        .fetch_optional(self.pool())
        .await?;

        match model {
            Some(model) => model.to_installed_app().map(Some).map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            }),
            None => Ok(None),
        }
    }

    /// Get all installed apps.
    pub async fn get_all_installed_apps(
        &self,
    ) -> sqlx::Result<Vec<(String, InstalledAppCommon, AppStatus)>> {
        let models: Vec<InstalledAppModel> = sqlx::query_as(
            "SELECT app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at FROM InstalledApp",
        )
        .fetch_all(self.pool())
        .await?;

        let mut apps = Vec::new();
        for model in models {
            let app_id = model.app_id.clone();
            let (app, status) = model.to_installed_app().map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })?;
            apps.push((app_id, app, status));
        }
        Ok(apps)
    }
}

// Write operations
impl DbWrite<Conductor> {
    /// Set the conductor tag.
    pub async fn set_conductor_tag(&self, tag: &str) -> sqlx::Result<()> {
        sqlx::query("INSERT INTO Conductor (id, tag) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET tag = excluded.tag")
            .bind(tag)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Insert or update an app interface.
    pub async fn put_app_interface(
        &self,
        port: i64,
        id: Option<&str>,
        model: &AppInterfaceModel,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO AppInterface (port, id, driver_type, websocket_port, danger_bind_addr, allowed_origins_blob, installed_app_id) 
             VALUES (?, ?, ?, ?, ?, ?, ?) 
             ON CONFLICT(port, id) DO UPDATE SET 
                driver_type = excluded.driver_type,
                websocket_port = excluded.websocket_port,
                danger_bind_addr = excluded.danger_bind_addr,
                allowed_origins_blob = excluded.allowed_origins_blob,
                installed_app_id = excluded.installed_app_id",
        )
        .bind(port)
        .bind(id)
        .bind(&model.driver_type)
        .bind(model.websocket_port)
        .bind(&model.danger_bind_addr)
        .bind(&model.allowed_origins_blob)
        .bind(&model.installed_app_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Save a signal subscription for an app interface.
    pub async fn put_signal_subscription(
        &self,
        interface_port: i64,
        interface_id: Option<&str>,
        app_id: &str,
        filters_blob: &[u8],
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO SignalSubscription (interface_port, interface_id, app_id, filters_blob) 
             VALUES (?, ?, ?, ?) 
             ON CONFLICT(interface_port, interface_id, app_id) DO UPDATE SET 
                filters_blob = excluded.filters_blob",
        )
        .bind(interface_port)
        .bind(interface_id)
        .bind(app_id)
        .bind(filters_blob)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Delete all signal subscriptions for an app interface.
    pub async fn delete_signal_subscriptions(
        &self,
        interface_port: i64,
        interface_id: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "DELETE FROM SignalSubscription WHERE interface_port = ? AND interface_id IS ?",
        )
        .bind(interface_port)
        .bind(interface_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Delete an app interface.
    pub async fn delete_app_interface(&self, port: i64, id: Option<&str>) -> sqlx::Result<()> {
        // Signal subscriptions will be deleted via CASCADE
        sqlx::query("DELETE FROM AppInterface WHERE port = ? AND id IS ?")
            .bind(port)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Insert or update an installed app.
    pub async fn put_installed_app(
        &self,
        app_id: &str,
        app: &InstalledAppCommon,
        status: &AppStatus,
    ) -> sqlx::Result<()> {
        let model = InstalledAppModel::from_installed_app(app_id, app, status).map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))
        })?;

        sqlx::query(
            "INSERT INTO InstalledApp (app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?) 
             ON CONFLICT(app_id) DO UPDATE SET 
                agent_pub_key = excluded.agent_pub_key,
                status = excluded.status,
                disabled_reason = excluded.disabled_reason,
                manifest_blob = excluded.manifest_blob,
                role_assignments_blob = excluded.role_assignments_blob,
                installed_at = excluded.installed_at",
        )
        .bind(&model.app_id)
        .bind(&model.agent_pub_key)
        .bind(&model.status)
        .bind(&model.disabled_reason)
        .bind(&model.manifest_blob)
        .bind(&model.role_assignments_blob)
        .bind(model.installed_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Delete an installed app.
    pub async fn delete_installed_app(&self, app_id: &str) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM InstalledApp WHERE app_id = ?")
            .bind(app_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind::Conductor, test_open_db};

    #[tokio::test]
    async fn test_conductor_schema_created() {
        let db = test_open_db(Conductor)
            .await
            .expect("Failed to set up test database");

        // Verify all tables were created
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('Conductor', 'InstalledApp', 'AppRole', 'CloneCell', 'AppInterface', 'SignalSubscription') ORDER BY name"
        )
        .fetch_all(db.pool())
        .await
        .expect("Failed to query tables");

        assert_eq!(tables.len(), 6);
        assert_eq!(tables[0], "AppInterface");
        assert_eq!(tables[1], "AppRole");
        assert_eq!(tables[2], "CloneCell");
        assert_eq!(tables[3], "Conductor");
        assert_eq!(tables[4], "InstalledApp");
        assert_eq!(tables[5], "SignalSubscription");
    }

    #[tokio::test]
    async fn test_conductor_table_singleton() {
        let db = test_open_db(Conductor)
            .await
            .expect("Failed to set up test database");

        // Insert a conductor tag
        sqlx::query("INSERT INTO Conductor (id, tag) VALUES (1, 'test-conductor')")
            .execute(db.pool())
            .await
            .expect("Failed to insert conductor");

        // Verify we can read it back
        let conductor: ConductorModel = sqlx::query_as("SELECT * FROM Conductor WHERE id = 1")
            .fetch_one(db.pool())
            .await
            .expect("Failed to fetch conductor");

        assert_eq!(conductor.id, 1);
        assert_eq!(conductor.tag, "test-conductor");

        // Verify singleton constraint - trying to insert id != 1 should fail
        let result = sqlx::query("INSERT INTO Conductor (id, tag) VALUES (2, 'another-tag')")
            .execute(db.pool())
            .await;

        assert!(result.is_err(), "Should not allow multiple conductors");
    }

    #[tokio::test]
    async fn test_installed_app_table() {
        let db = test_open_db(Conductor)
            .await
            .expect("Failed to set up test database");

        // Insert a test app
        let agent_key = vec![1u8; 36];
        let manifest = vec![0u8; 100];
        let role_assignments = vec![0u8; 10];
        sqlx::query(
            "INSERT INTO InstalledApp (app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("test-app")
        .bind(&agent_key)
        .bind("enabled")
        .bind(None::<String>)
        .bind(&manifest)
        .bind(&role_assignments)
        .bind(1234567890_i64)
        .execute(db.pool())
        .await
        .expect("Failed to insert app");

        // Verify we can read it back
        let app: InstalledAppModel = sqlx::query_as("SELECT * FROM InstalledApp WHERE app_id = ?")
            .bind("test-app")
            .fetch_one(db.pool())
            .await
            .expect("Failed to fetch app");

        assert_eq!(app.app_id, "test-app");
        assert_eq!(app.agent_pub_key, agent_key);
        assert_eq!(app.status, "enabled");
        assert_eq!(app.disabled_reason, None);
        assert_eq!(app.installed_at, 1234567890);

        // Test status constraint
        let result = sqlx::query(
            "INSERT INTO InstalledApp (app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("invalid-app")
        .bind(&agent_key)
        .bind("invalid-status")
        .bind(None::<String>)
        .bind(&manifest)
        .bind(&role_assignments)
        .bind(1234567890_i64)
        .execute(db.pool())
        .await;

        assert!(result.is_err(), "Should reject invalid status");
    }

    #[tokio::test]
    async fn test_app_role_foreign_key() {
        let db = test_open_db(Conductor)
            .await
            .expect("Failed to set up test database");

        let agent_key = vec![1u8; 36];
        let manifest = vec![0u8; 100];
        let role_assignments = vec![0u8; 10];
        let dna_hash = vec![2u8; 32];

        // Insert app first
        sqlx::query(
            "INSERT INTO InstalledApp (app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("test-app")
        .bind(&agent_key)
        .bind("enabled")
        .bind(None::<String>)
        .bind(&manifest)
        .bind(&role_assignments)
        .bind(1234567890_i64)
        .execute(db.pool())
        .await
        .expect("Failed to insert app");

        // Insert a role for the app
        sqlx::query(
            "INSERT INTO AppRole (app_id, role_name, dna_hash, is_clone_limit_enabled, clone_limit) 
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind("test-app")
        .bind("role1")
        .bind(&dna_hash)
        .bind(0)
        .bind(0)
        .execute(db.pool())
        .await
        .expect("Failed to insert role");

        // Verify we can read it back
        let role: AppRoleModel =
            sqlx::query_as("SELECT * FROM AppRole WHERE app_id = ? AND role_name = ?")
                .bind("test-app")
                .bind("role1")
                .fetch_one(db.pool())
                .await
                .expect("Failed to fetch role");

        assert_eq!(role.app_id, "test-app");
        assert_eq!(role.role_name, "role1");
        assert_eq!(role.dna_hash, dna_hash);

        // Test foreign key constraint - try to insert role for non-existent app
        let err = sqlx::query(
            "INSERT INTO AppRole (app_id, role_name, dna_hash, is_clone_limit_enabled, clone_limit) 
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind("non-existent-app")
        .bind("role2")
        .bind(&dna_hash)
        .bind(0)
        .bind(0)
        .execute(db.pool())
        .await
        .unwrap_err();

        let err_msg = err.to_string();
        assert!(
            err_msg.contains("FOREIGN KEY") || err_msg.contains("foreign key"),
            "Expected foreign key error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_cascade_delete() {
        let db = test_open_db(Conductor)
            .await
            .expect("Failed to set up test database");

        let agent_key = vec![1u8; 36];
        let manifest = vec![0u8; 100];
        let role_assignments = vec![0u8; 10];
        let dna_hash = vec![2u8; 32];

        // Insert app
        sqlx::query(
            "INSERT INTO InstalledApp (app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("test-app")
        .bind(&agent_key)
        .bind("enabled")
        .bind(None::<String>)
        .bind(&manifest)
        .bind(&role_assignments)
        .bind(1234567890_i64)
        .execute(db.pool())
        .await
        .expect("Failed to insert app");

        // Insert role
        sqlx::query(
            "INSERT INTO AppRole (app_id, role_name, dna_hash, is_clone_limit_enabled, clone_limit) 
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind("test-app")
        .bind("role1")
        .bind(&dna_hash)
        .bind(0)
        .bind(0)
        .execute(db.pool())
        .await
        .expect("Failed to insert role");

        // Insert clone cell
        sqlx::query(
            "INSERT INTO CloneCell (app_id, role_name, clone_id, dna_hash, is_enabled) 
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind("test-app")
        .bind("role1")
        .bind("clone1")
        .bind(&dna_hash)
        .bind(1)
        .execute(db.pool())
        .await
        .expect("Failed to insert clone cell");

        // Verify all exists
        let app_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM InstalledApp")
            .fetch_one(db.pool())
            .await
            .expect("Failed to count apps");
        assert_eq!(app_count, 1);

        let role_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM AppRole")
            .fetch_one(db.pool())
            .await
            .expect("Failed to count roles");
        assert_eq!(role_count, 1);

        let clone_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM CloneCell")
            .fetch_one(db.pool())
            .await
            .expect("Failed to count clones");
        assert_eq!(clone_count, 1);

        // Delete the app
        sqlx::query("DELETE FROM InstalledApp WHERE app_id = ?")
            .bind("test-app")
            .execute(db.pool())
            .await
            .expect("Failed to delete app");

        // Verify cascade delete removed roles and clone cells
        let role_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM AppRole")
            .fetch_one(db.pool())
            .await
            .expect("Failed to count roles after delete");
        assert_eq!(role_count, 0, "Role should be cascade deleted");

        let clone_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM CloneCell")
            .fetch_one(db.pool())
            .await
            .expect("Failed to count clones after delete");
        assert_eq!(clone_count, 0, "Clone cell should be cascade deleted");
    }
}

// ============================================================================
// Nonce Witnessing Operations
// ============================================================================

/// Maximum duration a nonce can be valid for
pub const WITNESSABLE_EXPIRY_DURATION: std::time::Duration =
    std::time::Duration::from_secs(60 * 50);

#[derive(PartialEq, Debug)]
pub enum WitnessNonceResult {
    Fresh,
    Duplicate,
    Expired,
    Future,
}

impl DbRead<Conductor> {
    /// Check if a nonce has already been seen
    pub async fn nonce_already_seen(
        &self,
        agent: &AgentPubKey,
        nonce: &Nonce256Bits,
        now: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        let agent_bytes = agent.get_raw_36();
        let nonce_bytes = nonce.as_ref();
        let now_micros = now.as_micros();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(1) FROM Nonce WHERE agent = ? AND nonce = ? AND expires > ?",
        )
        .bind(agent_bytes)
        .bind(nonce_bytes)
        .bind(now_micros)
        .fetch_one(self.pool())
        .await?;

        Ok(count > 0)
    }
}

impl DbWrite<Conductor> {
    /// Witness a nonce (check if it's fresh and record it)
    pub async fn witness_nonce(
        &self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        now: Timestamp,
        expires: Timestamp,
    ) -> Result<WitnessNonceResult, sqlx::Error> {
        // Treat expired but also very far future expiries as stale as we cannot trust the time
        if expires <= now {
            return Ok(WitnessNonceResult::Expired);
        }

        let future_limit = (now + WITNESSABLE_EXPIRY_DURATION)
            .map_err(|_| sqlx::Error::Protocol("Timestamp overflow".to_string()))?;

        if expires > future_limit {
            return Ok(WitnessNonceResult::Future);
        }

        // Delete expired nonces first
        sqlx::query("DELETE FROM Nonce WHERE expires <= ?")
            .bind(now.as_micros())
            .execute(self.pool())
            .await?;

        // Check if already seen
        let db_read: &DbRead<Conductor> = self.as_ref();
        if db_read.nonce_already_seen(&agent, &nonce, now).await? {
            return Ok(WitnessNonceResult::Duplicate);
        }

        // Insert the new nonce
        let agent_bytes = agent.get_raw_36();
        let nonce_bytes = nonce.as_ref();

        sqlx::query(
            "INSERT INTO Nonce (agent, nonce, expires) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(agent_bytes)
        .bind(nonce_bytes)
        .bind(expires.as_micros())
        .execute(self.pool())
        .await?;

        Ok(WitnessNonceResult::Fresh)
    }
}

// ============================================================================
// Block/Unblock Operations
// ============================================================================

impl DbRead<Conductor> {
    /// Check whether a given target is blocked at the given time
    pub async fn is_blocked(
        &self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        let target_bytes = SerializedBytes::try_from(target_id)
            .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?
            .bytes()
            .to_vec();
        let time_micros = timestamp.as_micros();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(1) > 0 FROM BlockSpan WHERE target_id = ? AND start_us <= ? AND ? <= end_us",
        )
        .bind(&target_bytes)
        .bind(time_micros)
        .bind(time_micros)
        .fetch_one(self.pool())
        .await?;

        Ok(count > 0)
    }

    /// Query whether all BlockTargetIds in the provided vector are blocked at the given timestamp
    pub async fn are_all_blocked(
        &self,
        target_ids: Vec<BlockTargetId>,
        timestamp: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        // If no targets provided, return false
        if target_ids.is_empty() {
            return Ok(false);
        }

        // Deduplicate to ensure duplicates don't cause false negatives
        let unique_ids: HashSet<BlockTargetId> = target_ids.into_iter().collect();
        let ids_len = unique_ids.len() as i64;
        let time_micros = timestamp.as_micros();

        // For simplicity, check each target_id individually and ensure all are blocked
        let mut all_blocked = true;
        for target_id in unique_ids.into_iter() {
            let target_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_id)
                 .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(1) > 0 FROM BlockSpan WHERE target_id = ? AND start_us <= ? AND ? <= end_us",
            )
            .bind(&target_bytes)
            .bind(time_micros)
            .bind(time_micros)
            .fetch_one(self.pool())
            .await?;

            if count == 0 {
                all_blocked = false;
                break;
            }
        }

        Ok(all_blocked && ids_len > 0)
    }
}

impl DbWrite<Conductor> {
    /// Get overlapping block bounds for merging
    async fn pluck_overlapping_block_bounds(
        &self,
        block: &Block,
    ) -> Result<(Option<i64>, Option<i64>), sqlx::Error> {
        let target_id = BlockTargetId::from(block.target().clone());
        let target_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_id)
             .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;
        let start_us = block.start().as_micros();
        let end_us = block.end().as_micros();

        let result: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT MIN(start_us), MAX(end_us) FROM BlockSpan \
             WHERE target_id = ? AND start_us <= ? AND ? <= end_us",
        )
        .bind(&target_bytes)
        .bind(end_us)
        .bind(start_us)
        .fetch_optional(self.pool())
        .await?;

        Ok(result.unwrap_or((None, None)))
    }

    /// Insert a block span into the database
    async fn insert_block_inner(&self, block: Block) -> Result<(), sqlx::Error> {
        let target_id = BlockTargetId::from(block.target().clone());
        let target_id_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_id)
             .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;
        use holochain_zome_types::block::BlockTargetReason;
        let target_reason = BlockTargetReason::from(block.target().clone());
        let target_reason_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_reason)
             .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

        sqlx::query(
            "INSERT INTO BlockSpan (target_id, target_reason, start_us, end_us) VALUES (?, ?, ?, ?)",
        )
        .bind(target_id_bytes)
        .bind(target_reason_bytes)
        .bind(block.start().as_micros())
        .bind(block.end().as_micros())
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Insert a block into the database, merging with overlapping blocks
    pub async fn block(&self, input: Block) -> Result<(), sqlx::Error> {
        let maybe_min_maybe_max = self.pluck_overlapping_block_bounds(&input).await?;

        // Delete overlapping blocks
        let target_id = BlockTargetId::from(input.target().clone());
        let target_id_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_id)
             .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

        sqlx::query("DELETE FROM BlockSpan WHERE target_id = ? AND start_us <= ? AND ? <= end_us")
            .bind(&target_id_bytes)
            .bind(input.end().as_micros())
            .bind(input.start().as_micros())
            .execute(self.pool())
            .await?;

        // Build one new block from the extremums
        let merged_block = Block::new(
            input.target().clone(),
            InclusiveTimestampInterval::try_new(
                maybe_min_maybe_max
                    .0
                    .map(Timestamp)
                    .map(|min| std::cmp::min(min, input.start()))
                    .unwrap_or(input.start()),
                maybe_min_maybe_max
                    .1
                    .map(Timestamp)
                    .map(|max| std::cmp::max(max, input.end()))
                    .unwrap_or(input.end()),
            )
            .map_err(|e| sqlx::Error::Protocol(format!("Timestamp error: {}", e)))?,
        );

        self.insert_block_inner(merged_block).await
    }

    /// Insert an unblock into the database, splitting existing blocks as needed
    pub async fn unblock(&self, unblock: Block) -> Result<(), sqlx::Error> {
        let maybe_min_maybe_max = self.pluck_overlapping_block_bounds(&unblock).await?;

        // Delete overlapping blocks
        let target_id = BlockTargetId::from(unblock.target().clone());
        let target_id_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_id)
             .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

         sqlx::query("DELETE FROM BlockSpan WHERE target_id = ? AND start_us <= ? AND ? <= end_us")
            .bind(&target_id_bytes)
            .bind(unblock.end().as_micros())
            .bind(unblock.start().as_micros())
            .execute(self.pool())
            .await?;

        // Reinstate anything before the unblock
        if let (Some(min), _) = maybe_min_maybe_max {
            let preblock_start = Timestamp(min);
            // Unblocks are inclusive so we reinstate the preblock up to but not including the unblock start
             if let Ok(preblock_end) = unblock.start() - std::time::Duration::from_micros(1) {
                    if preblock_start <= preblock_end {
                     self.insert_block_inner(Block::new(
                         unblock.target().clone(),
                         InclusiveTimestampInterval::try_new(preblock_start, preblock_end)
                             .map_err(|e| {
                                 sqlx::Error::Protocol(format!("Timestamp error: {}", e))
                             })?,
                     ))
                     .await?;
                 }
            }
        }

        // Reinstate anything after the unblock
        if let (_, Some(max)) = maybe_min_maybe_max {
            let postblock_end = Timestamp(max);
             if let Ok(postblock_start) = unblock.end() + std::time::Duration::from_micros(1) {
                    if postblock_start <= postblock_end {
                     self.insert_block_inner(Block::new(
                         unblock.target().clone(),
                         InclusiveTimestampInterval::try_new(postblock_start, postblock_end)
                             .map_err(|e| {
                                 sqlx::Error::Protocol(format!("Timestamp error: {}", e))
                             })?,
                     ))
                     .await?;
                 }
            }
        }

        Ok(())
    }
}
