//! Conductor database models and operations
//!
//! This module provides data models and database operations for the conductor database,
//! which stores the conductor's state including installed apps, roles, and interfaces.

use holochain_types::prelude::*;

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
            AppStatus::Disabled(reason) => ("disabled".to_string(), Some(format!("{:?}", reason))),
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
                // For now, we'll use a generic reason since we can't fully reconstruct the enum
                AppStatus::Disabled(DisabledAppReason::Error(reason_str.clone()))
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
    use crate::{kind::Conductor, test_setup_holochain_data};

    #[tokio::test]
    async fn test_conductor_schema_created() {
        let db = test_setup_holochain_data(Conductor)
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
        let db = test_setup_holochain_data(Conductor)
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
        let db = test_setup_holochain_data(Conductor)
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
        let db = test_setup_holochain_data(Conductor)
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
        let db = test_setup_holochain_data(Conductor)
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
