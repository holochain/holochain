//! Conductor database models.
//!
//! These models represent the conductor's state including installed apps,
//! roles, interfaces, and related metadata.

use holochain_conductor_api::config::InterfaceDriver;
use holochain_types::prelude::*;
use holochain_types::websocket::AllowedOrigins;

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
    pub id: String,
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
                    id: String::new(),
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

/// Maximum duration a nonce can be valid for
pub const WITNESSABLE_EXPIRY_DURATION: std::time::Duration =
    std::time::Duration::from_secs(60 * 50);

/// Result of witnessing a nonce
#[derive(PartialEq, Debug)]
pub enum WitnessNonceResult {
    Fresh,
    Duplicate,
    Expired,
    Future,
}
