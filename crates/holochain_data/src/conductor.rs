//! Conductor database operations.
//!
//! This module provides database operations for the conductor database,
//! which stores the conductor's state including installed apps, roles, and interfaces.

use holochain_types::prelude::*;

pub use crate::models::conductor::{
    AppInterfaceModel, AppRoleModel, CloneCellModel, ConductorModel, InstalledAppModel,
    WitnessNonceResult, WITNESSABLE_EXPIRY_DURATION,
};
pub use holochain_nonce::Nonce256Bits;
pub use holochain_timestamp::InclusiveTimestampInterval;
pub use holochain_zome_types::block::{Block, BlockTargetId};

use crate::handles::{DbRead, DbWrite, TxRead, TxWrite};
use crate::kind::Conductor;
use sqlx::{Acquire, Executor, Sqlite};

// ============================================================================
// Conductor / App / Interface operations
// ============================================================================

/// Get the conductor tag.
async fn get_conductor_tag<'e, E>(executor: E) -> sqlx::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let tag: Option<String> = sqlx::query_scalar("SELECT tag FROM Conductor WHERE id = 1")
        .fetch_optional(executor)
        .await?;
    Ok(tag)
}

/// Get all app interfaces.
async fn get_all_app_interfaces<'e, E>(executor: E) -> sqlx::Result<Vec<AppInterfaceModel>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let models: Vec<AppInterfaceModel> = sqlx::query_as(
        "SELECT port, id, driver_type, websocket_port, danger_bind_addr, allowed_origins_blob, installed_app_id FROM AppInterface",
    )
    .fetch_all(executor)
    .await?;

    Ok(models)
}

/// Get signal subscriptions for a specific app interface.
async fn get_signal_subscriptions<'e, E>(
    executor: E,
    port: i64,
    id: &str,
) -> sqlx::Result<Vec<(String, Vec<u8>)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    #[derive(sqlx::FromRow)]
    struct Row {
        app_id: String,
        filters_blob: Option<Vec<u8>>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT app_id, filters_blob FROM SignalSubscription WHERE interface_port = ? AND interface_id = ?",
    )
    .bind(port)
    .bind(id)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| r.filters_blob.map(|blob| (r.app_id, blob)))
        .collect())
}

/// Get an installed app by ID.
async fn get_installed_app<'e, E>(
    executor: E,
    app_id: &str,
) -> sqlx::Result<Option<(InstalledAppCommon, AppStatus)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let model: Option<InstalledAppModel> = sqlx::query_as(
        "SELECT app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at FROM InstalledApp WHERE app_id = ?",
    )
    .bind(app_id)
    .fetch_optional(executor)
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
async fn get_all_installed_apps<'e, E>(
    executor: E,
) -> sqlx::Result<Vec<(String, InstalledAppCommon, AppStatus)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let models: Vec<InstalledAppModel> = sqlx::query_as(
        "SELECT app_id, agent_pub_key, status, disabled_reason, manifest_blob, role_assignments_blob, installed_at FROM InstalledApp",
    )
    .fetch_all(executor)
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

/// Set the conductor tag.
async fn set_conductor_tag<'e, E>(executor: E, tag: &str) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO Conductor (id, tag) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET tag = excluded.tag")
        .bind(tag)
        .execute(executor)
        .await?;
    Ok(())
}

/// Insert or update an app interface.
async fn put_app_interface<'e, E>(
    executor: E,
    port: i64,
    id: &str,
    model: &AppInterfaceModel,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
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
    .execute(executor)
    .await?;
    Ok(())
}

/// Save a signal subscription for an app interface.
async fn put_signal_subscription<'e, E>(
    executor: E,
    interface_port: i64,
    interface_id: &str,
    app_id: &str,
    filters_blob: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
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
    .execute(executor)
    .await?;
    Ok(())
}

/// Delete all signal subscriptions for an app interface.
async fn delete_signal_subscriptions<'e, E>(
    executor: E,
    interface_port: i64,
    interface_id: &str,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM SignalSubscription WHERE interface_port = ? AND interface_id = ?")
        .bind(interface_port)
        .bind(interface_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Delete an app interface.
async fn delete_app_interface<'e, E>(executor: E, port: i64, id: &str) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    // Signal subscriptions will be deleted via CASCADE
    sqlx::query("DELETE FROM AppInterface WHERE port = ? AND id = ?")
        .bind(port)
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Insert or update an installed app.
async fn put_installed_app<'e, E>(
    executor: E,
    app_id: &str,
    app: &InstalledAppCommon,
    status: &AppStatus,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
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
    .execute(executor)
    .await?;
    Ok(())
}

/// Delete an installed app.
async fn delete_installed_app<'e, E>(executor: E, app_id: &str) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM InstalledApp WHERE app_id = ?")
        .bind(app_id)
        .execute(executor)
        .await?;
    Ok(())
}

// ============================================================================
// Nonce Witnessing Operations
// ============================================================================

/// Check if a nonce has already been seen
async fn nonce_already_seen<'e, E>(
    executor: E,
    agent: &AgentPubKey,
    nonce: &Nonce256Bits,
    now: Timestamp,
) -> Result<bool, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let agent_bytes = agent.get_raw_36();
    let nonce_bytes = nonce.as_ref();
    let now_micros = now.as_micros();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM Nonce WHERE agent = ? AND nonce = ? AND expires > ?",
    )
    .bind(agent_bytes)
    .bind(nonce_bytes)
    .bind(now_micros)
    .fetch_one(executor)
    .await?;

    Ok(count > 0)
}

/// Witness a nonce (check if it's fresh and record it).
///
/// Acquires a single connection and runs the DELETE-expired, duplicate-check,
/// and INSERT sequence on it.
async fn witness_nonce<'c, A>(
    conn: A,
    agent: AgentPubKey,
    nonce: Nonce256Bits,
    now: Timestamp,
    expires: Timestamp,
) -> Result<WitnessNonceResult, sqlx::Error>
where
    A: Acquire<'c, Database = Sqlite>,
{
    // Treat expired but also very far future expiries as stale as we cannot trust the time
    if expires <= now {
        return Ok(WitnessNonceResult::Expired);
    }

    let future_limit = (now + WITNESSABLE_EXPIRY_DURATION)
        .map_err(|_| sqlx::Error::Protocol("Timestamp overflow".to_string()))?;

    if expires > future_limit {
        return Ok(WitnessNonceResult::Future);
    }

    let mut conn = conn.acquire().await?;

    // Delete expired nonces first
    sqlx::query("DELETE FROM Nonce WHERE expires <= ?")
        .bind(now.as_micros())
        .execute(&mut *conn)
        .await?;

    // Attempt to record the nonce; rely on the insert's rows_affected to
    // determine the outcome so concurrent callers never both see Fresh.
    // ON CONFLICT DO NOTHING on the (agent, nonce) primary key makes this
    // a single atomic "claim or fail" regardless of interleaving.
    let agent_bytes = agent.get_raw_36();
    let nonce_bytes = nonce.as_ref();

    let result = sqlx::query(
        "INSERT INTO Nonce (agent, nonce, expires) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(agent_bytes)
    .bind(nonce_bytes)
    .bind(expires.as_micros())
    .execute(&mut *conn)
    .await?;

    if result.rows_affected() == 0 {
        Ok(WitnessNonceResult::Duplicate)
    } else {
        Ok(WitnessNonceResult::Fresh)
    }
}

// ============================================================================
// Block/Unblock Operations
// ============================================================================

/// Check whether a given target is blocked at the given time
async fn is_blocked<'e, E>(
    executor: E,
    target_id: BlockTargetId,
    timestamp: Timestamp,
) -> Result<bool, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let target_bytes = holochain_serialized_bytes::encode(&target_id)
        .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;
    let time_micros = timestamp.as_micros();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) > 0 FROM BlockSpan WHERE target_id = ? AND start_us <= ? AND ? <= end_us",
    )
    .bind(&target_bytes)
    .bind(time_micros)
    .bind(time_micros)
    .fetch_one(executor)
    .await?;

    Ok(count > 0)
}

/// Query whether any BlockTargetId in the provided vector is blocked at the given timestamp.
///
/// Acquires a single connection for the loop of per-target queries.
async fn is_any_blocked<'c, A>(
    conn: A,
    target_ids: Vec<BlockTargetId>,
    timestamp: Timestamp,
) -> Result<bool, sqlx::Error>
where
    A: Acquire<'c, Database = Sqlite>,
{
    if target_ids.is_empty() {
        return Ok(false);
    }

    let mut conn = conn.acquire().await?;
    let time_micros = timestamp.as_micros();

    for target_id in target_ids {
        let target_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_id)
            .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;
        let found: i64 = sqlx::query_scalar(
            "SELECT COUNT(1) > 0 FROM BlockSpan WHERE target_id = ? AND start_us <= ? AND ? <= end_us",
        )
        .bind(&target_bytes)
        .bind(time_micros)
        .bind(time_micros)
        .fetch_one(&mut *conn)
        .await?;

        if found != 0 {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Get all blocks from the database.
async fn get_all_blocks<'e, E>(executor: E) -> Result<Vec<Block>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    use holochain_zome_types::block::{BlockTarget, BlockTargetReason};

    let rows: Vec<(Vec<u8>, Vec<u8>, i64, i64)> =
        sqlx::query_as("SELECT target_id, target_reason, start_us, end_us FROM BlockSpan")
            .fetch_all(executor)
            .await?;

    let mut blocks = Vec::with_capacity(rows.len());
    for (target_id_bytes, target_reason_bytes, start_us, end_us) in rows {
        let target_id: BlockTargetId = holochain_serialized_bytes::decode(&target_id_bytes)
            .map_err(|e| sqlx::Error::Protocol(format!("Deserialization error: {}", e)))?;
        let target_reason: BlockTargetReason =
            holochain_serialized_bytes::decode(&target_reason_bytes)
                .map_err(|e| sqlx::Error::Protocol(format!("Deserialization error: {}", e)))?;

        let target = match (target_id, target_reason) {
            (BlockTargetId::Cell(cell_id), BlockTargetReason::Cell(reason)) => {
                BlockTarget::Cell(cell_id, reason)
            }
            (BlockTargetId::Ip(ip), BlockTargetReason::Ip(reason)) => BlockTarget::Ip(ip, reason),
            _ => {
                return Err(sqlx::Error::Protocol(
                    "Mismatched block target id and reason".to_string(),
                ));
            }
        };

        let interval = InclusiveTimestampInterval::try_new(
            Timestamp::from_micros(start_us),
            Timestamp::from_micros(end_us),
        )
        .map_err(|e| sqlx::Error::Protocol(format!("Invalid timestamp interval: {:?}", e)))?;

        blocks.push(Block::new(target, interval));
    }

    Ok(blocks)
}

/// Append a block to the database.
///
/// Blocks are stored as independent rows and never merged. Overlapping blocks
/// on the same target accumulate as separate rows; `is_blocked` returns true
/// if any row covers the queried timestamp.
async fn block<'e, E>(executor: E, input: Block) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    use holochain_zome_types::block::BlockTargetReason;

    let target_id = BlockTargetId::from(input.target().clone());
    let target_id_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_id)
        .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;
    let target_reason = BlockTargetReason::from(input.target().clone());
    let target_reason_bytes: Vec<u8> = holochain_serialized_bytes::encode(&target_reason)
        .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

    sqlx::query(
        "INSERT INTO BlockSpan (target_id, target_reason, start_us, end_us) VALUES (?, ?, ?, ?)",
    )
    .bind(target_id_bytes)
    .bind(target_reason_bytes)
    .bind(input.start().as_micros())
    .bind(input.end().as_micros())
    .execute(executor)
    .await?;

    Ok(())
}

// ============================================================================
// DbRead / DbWrite wrappers
// ============================================================================

impl DbRead<Conductor> {
    /// Get the conductor tag.
    pub async fn get_conductor_tag(&self) -> sqlx::Result<Option<String>> {
        get_conductor_tag(self.pool()).await
    }

    /// Get all app interfaces.
    pub async fn get_all_app_interfaces(&self) -> sqlx::Result<Vec<AppInterfaceModel>> {
        get_all_app_interfaces(self.pool()).await
    }

    /// Get signal subscriptions for a specific app interface.
    pub async fn get_signal_subscriptions(
        &self,
        port: i64,
        id: &str,
    ) -> sqlx::Result<Vec<(String, Vec<u8>)>> {
        get_signal_subscriptions(self.pool(), port, id).await
    }

    /// Get an installed app by ID.
    pub async fn get_installed_app(
        &self,
        app_id: &str,
    ) -> sqlx::Result<Option<(InstalledAppCommon, AppStatus)>> {
        get_installed_app(self.pool(), app_id).await
    }

    /// Get all installed apps.
    pub async fn get_all_installed_apps(
        &self,
    ) -> sqlx::Result<Vec<(String, InstalledAppCommon, AppStatus)>> {
        get_all_installed_apps(self.pool()).await
    }

    /// Check if a nonce has already been seen
    pub async fn nonce_already_seen(
        &self,
        agent: &AgentPubKey,
        nonce: &Nonce256Bits,
        now: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        nonce_already_seen(self.pool(), agent, nonce, now).await
    }

    /// Check whether a given target is blocked at the given time
    pub async fn is_blocked(
        &self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        is_blocked(self.pool(), target_id, timestamp).await
    }

    /// Query whether any BlockTargetId in the provided vector is blocked at the given timestamp
    pub async fn is_any_blocked(
        &self,
        target_ids: Vec<BlockTargetId>,
        timestamp: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        is_any_blocked(self.pool(), target_ids, timestamp).await
    }

    /// Get all blocks from the database.
    pub async fn get_all_blocks(&self) -> Result<Vec<Block>, sqlx::Error> {
        get_all_blocks(self.pool()).await
    }
}

impl DbWrite<Conductor> {
    /// Set the conductor tag.
    pub async fn set_conductor_tag(&self, tag: &str) -> sqlx::Result<()> {
        set_conductor_tag(self.pool(), tag).await
    }

    /// Insert or update an app interface.
    pub async fn put_app_interface(
        &self,
        port: i64,
        id: &str,
        model: &AppInterfaceModel,
    ) -> sqlx::Result<()> {
        put_app_interface(self.pool(), port, id, model).await
    }

    /// Save a signal subscription for an app interface.
    pub async fn put_signal_subscription(
        &self,
        interface_port: i64,
        interface_id: &str,
        app_id: &str,
        filters_blob: &[u8],
    ) -> sqlx::Result<()> {
        put_signal_subscription(
            self.pool(),
            interface_port,
            interface_id,
            app_id,
            filters_blob,
        )
        .await
    }

    /// Delete all signal subscriptions for an app interface.
    pub async fn delete_signal_subscriptions(
        &self,
        interface_port: i64,
        interface_id: &str,
    ) -> sqlx::Result<()> {
        delete_signal_subscriptions(self.pool(), interface_port, interface_id).await
    }

    /// Delete an app interface.
    pub async fn delete_app_interface(&self, port: i64, id: &str) -> sqlx::Result<()> {
        delete_app_interface(self.pool(), port, id).await
    }

    /// Insert or update an installed app.
    pub async fn put_installed_app(
        &self,
        app_id: &str,
        app: &InstalledAppCommon,
        status: &AppStatus,
    ) -> sqlx::Result<()> {
        put_installed_app(self.pool(), app_id, app, status).await
    }

    /// Delete an installed app.
    pub async fn delete_installed_app(&self, app_id: &str) -> sqlx::Result<()> {
        delete_installed_app(self.pool(), app_id).await
    }

    /// Witness a nonce (check if it's fresh and record it)
    pub async fn witness_nonce(
        &self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        now: Timestamp,
        expires: Timestamp,
    ) -> Result<WitnessNonceResult, sqlx::Error> {
        witness_nonce(self.pool(), agent, nonce, now, expires).await
    }

    /// Append a block to the database.
    pub async fn block(&self, input: Block) -> Result<(), sqlx::Error> {
        block(self.pool(), input).await
    }
}

impl TxRead<Conductor> {
    /// Get the conductor tag.
    pub async fn get_conductor_tag(&mut self) -> sqlx::Result<Option<String>> {
        get_conductor_tag(self.conn_mut()).await
    }

    /// Get all app interfaces.
    pub async fn get_all_app_interfaces(&mut self) -> sqlx::Result<Vec<AppInterfaceModel>> {
        get_all_app_interfaces(self.conn_mut()).await
    }

    /// Get signal subscriptions for a specific app interface.
    pub async fn get_signal_subscriptions(
        &mut self,
        port: i64,
        id: &str,
    ) -> sqlx::Result<Vec<(String, Vec<u8>)>> {
        get_signal_subscriptions(self.conn_mut(), port, id).await
    }

    /// Get an installed app by ID.
    pub async fn get_installed_app(
        &mut self,
        app_id: &str,
    ) -> sqlx::Result<Option<(InstalledAppCommon, AppStatus)>> {
        get_installed_app(self.conn_mut(), app_id).await
    }

    /// Get all installed apps.
    pub async fn get_all_installed_apps(
        &mut self,
    ) -> sqlx::Result<Vec<(String, InstalledAppCommon, AppStatus)>> {
        get_all_installed_apps(self.conn_mut()).await
    }

    /// Check if a nonce has already been seen.
    pub async fn nonce_already_seen(
        &mut self,
        agent: &AgentPubKey,
        nonce: &Nonce256Bits,
        now: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        nonce_already_seen(self.conn_mut(), agent, nonce, now).await
    }

    /// Check whether a given target is blocked at the given time.
    pub async fn is_blocked(
        &mut self,
        target_id: BlockTargetId,
        timestamp: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        is_blocked(self.conn_mut(), target_id, timestamp).await
    }

    /// Query whether any BlockTargetId in the provided vector is blocked at the given timestamp.
    pub async fn is_any_blocked(
        &mut self,
        target_ids: Vec<BlockTargetId>,
        timestamp: Timestamp,
    ) -> Result<bool, sqlx::Error> {
        is_any_blocked(self.tx_mut(), target_ids, timestamp).await
    }

    /// Get all blocks from the database.
    pub async fn get_all_blocks(&mut self) -> Result<Vec<Block>, sqlx::Error> {
        get_all_blocks(self.conn_mut()).await
    }
}

impl TxWrite<Conductor> {
    /// Set the conductor tag.
    pub async fn set_conductor_tag(&mut self, tag: &str) -> sqlx::Result<()> {
        set_conductor_tag(self.conn_mut(), tag).await
    }

    /// Insert or update an app interface.
    pub async fn put_app_interface(
        &mut self,
        port: i64,
        id: &str,
        model: &AppInterfaceModel,
    ) -> sqlx::Result<()> {
        put_app_interface(self.conn_mut(), port, id, model).await
    }

    /// Save a signal subscription for an app interface.
    pub async fn put_signal_subscription(
        &mut self,
        interface_port: i64,
        interface_id: &str,
        app_id: &str,
        filters_blob: &[u8],
    ) -> sqlx::Result<()> {
        put_signal_subscription(
            self.conn_mut(),
            interface_port,
            interface_id,
            app_id,
            filters_blob,
        )
        .await
    }

    /// Delete all signal subscriptions for an app interface.
    pub async fn delete_signal_subscriptions(
        &mut self,
        interface_port: i64,
        interface_id: &str,
    ) -> sqlx::Result<()> {
        delete_signal_subscriptions(self.conn_mut(), interface_port, interface_id).await
    }

    /// Delete an app interface.
    pub async fn delete_app_interface(&mut self, port: i64, id: &str) -> sqlx::Result<()> {
        delete_app_interface(self.conn_mut(), port, id).await
    }

    /// Insert or update an installed app.
    pub async fn put_installed_app(
        &mut self,
        app_id: &str,
        app: &InstalledAppCommon,
        status: &AppStatus,
    ) -> sqlx::Result<()> {
        put_installed_app(self.conn_mut(), app_id, app, status).await
    }

    /// Delete an installed app.
    pub async fn delete_installed_app(&mut self, app_id: &str) -> sqlx::Result<()> {
        delete_installed_app(self.conn_mut(), app_id).await
    }

    /// Witness a nonce (check if it's fresh and record it).
    pub async fn witness_nonce(
        &mut self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        now: Timestamp,
        expires: Timestamp,
    ) -> Result<WitnessNonceResult, sqlx::Error> {
        witness_nonce(self.tx_mut(), agent, nonce, now, expires).await
    }

    /// Append a block to the database.
    pub async fn block(&mut self, input: Block) -> Result<(), sqlx::Error> {
        block(self.conn_mut(), input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::DbRead;
    use crate::test_open_db;
    use holo_hash::{AgentPubKey, DnaHash};
    use holochain_zome_types::block::{BlockTarget, CellBlockReason};
    use holochain_zome_types::cell::CellId;

    #[tokio::test]
    async fn conductor_schema_created() {
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
    async fn conductor_table_singleton() {
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
    async fn installed_app_table() {
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
    }

    #[tokio::test]
    async fn app_role_foreign_key() {
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
             VALUES (?, ?, ?, ?, ?)",
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
             VALUES (?, ?, ?, ?, ?)",
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
    async fn cascade_delete() {
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
             VALUES (?, ?, ?, ?, ?)",
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

    // ========================================================================
    // Conductor tag operations
    // ========================================================================

    #[tokio::test]
    async fn conductor_tag_roundtrip() {
        let db = test_open_db(Conductor).await.unwrap();

        // No tag initially
        assert_eq!(db.as_ref().get_conductor_tag().await.unwrap(), None);

        // Set a tag
        db.set_conductor_tag("my-conductor").await.unwrap();
        assert_eq!(
            db.as_ref().get_conductor_tag().await.unwrap(),
            Some("my-conductor".to_string())
        );

        // Upsert overwrites
        db.set_conductor_tag("updated-tag").await.unwrap();
        assert_eq!(
            db.as_ref().get_conductor_tag().await.unwrap(),
            Some("updated-tag".to_string())
        );
    }

    // ========================================================================
    // App interface operations
    // ========================================================================

    #[tokio::test]
    async fn app_interface_roundtrip() {
        let db = test_open_db(Conductor).await.unwrap();

        let model = AppInterfaceModel {
            port: 8080,
            id: "iface-1".to_string(),
            driver_type: "websocket".to_string(),
            websocket_port: Some(8080),
            danger_bind_addr: None,
            allowed_origins_blob: None,
            installed_app_id: None,
        };

        db.put_app_interface(8080, "iface-1", &model).await.unwrap();

        let interfaces = db.as_ref().get_all_app_interfaces().await.unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].port, 8080);
        assert_eq!(interfaces[0].id, "iface-1");

        // Delete it
        db.delete_app_interface(8080, "iface-1").await.unwrap();
        let interfaces = db.as_ref().get_all_app_interfaces().await.unwrap();
        assert!(interfaces.is_empty());
    }

    // ========================================================================
    // Signal subscription operations
    // ========================================================================

    #[tokio::test]
    async fn signal_subscription_roundtrip() {
        let db = test_open_db(Conductor).await.unwrap();

        // Need an app and interface first (foreign keys)
        let agent_key = vec![1u8; 36];
        let manifest = vec![0u8; 10];
        let role_assignments = b"{}".to_vec();
        sqlx::query(
            "INSERT INTO InstalledApp (app_id, agent_pub_key, status, manifest_blob, role_assignments_blob, installed_at)
             VALUES (?, ?, 'enabled', ?, ?, ?)",
        )
        .bind("app-1")
        .bind(&agent_key)
        .bind(&manifest)
        .bind(&role_assignments)
        .bind(1000_i64)
        .execute(db.pool())
        .await
        .unwrap();

        let iface = AppInterfaceModel {
            port: 9090,
            id: String::new(),
            driver_type: "websocket".to_string(),
            websocket_port: Some(9090),
            danger_bind_addr: None,
            allowed_origins_blob: None,
            installed_app_id: None,
        };
        db.put_app_interface(9090, "", &iface).await.unwrap();

        // Add a subscription
        let filters = b"some-filters";
        db.put_signal_subscription(9090, "", "app-1", filters)
            .await
            .unwrap();

        let subs = db
            .as_ref()
            .get_signal_subscriptions(9090, "")
            .await
            .unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].0, "app-1");
        assert_eq!(subs[0].1, filters);

        // Delete subscriptions
        db.delete_signal_subscriptions(9090, "").await.unwrap();
        let subs = db
            .as_ref()
            .get_signal_subscriptions(9090, "")
            .await
            .unwrap();
        assert!(subs.is_empty());
    }

    // ========================================================================
    // Nonce witnessing operations
    // ========================================================================

    fn test_agent() -> AgentPubKey {
        AgentPubKey::from_raw_36(vec![1u8; 36])
    }

    fn test_nonce(seed: u8) -> Nonce256Bits {
        Nonce256Bits::from([seed; 32])
    }

    #[tokio::test]
    async fn witness_nonce_fresh() {
        let db = test_open_db(Conductor).await.unwrap();
        let agent = test_agent();
        let nonce = test_nonce(1);
        let now = Timestamp::from_micros(1_000_000);
        let expires = Timestamp::from_micros(2_000_000);

        let result = db.witness_nonce(agent, nonce, now, expires).await.unwrap();
        assert_eq!(result, WitnessNonceResult::Fresh);
    }

    #[tokio::test]
    async fn witness_nonce_duplicate() {
        let db = test_open_db(Conductor).await.unwrap();
        let agent = test_agent();
        let nonce = test_nonce(2);
        let now = Timestamp::from_micros(1_000_000);
        let expires = Timestamp::from_micros(2_000_000);

        let r1 = db
            .witness_nonce(agent.clone(), nonce, now, expires)
            .await
            .unwrap();
        assert_eq!(r1, WitnessNonceResult::Fresh);

        let r2 = db.witness_nonce(agent, nonce, now, expires).await.unwrap();
        assert_eq!(r2, WitnessNonceResult::Duplicate);
    }

    #[tokio::test]
    async fn witness_nonce_expired() {
        let db = test_open_db(Conductor).await.unwrap();
        let agent = test_agent();
        let nonce = test_nonce(3);
        let now = Timestamp::from_micros(5_000_000);
        let expires = Timestamp::from_micros(1_000_000); // in the past

        let result = db.witness_nonce(agent, nonce, now, expires).await.unwrap();
        assert_eq!(result, WitnessNonceResult::Expired);
    }

    #[tokio::test]
    async fn witness_nonce_future() {
        let db = test_open_db(Conductor).await.unwrap();
        let agent = test_agent();
        let nonce = test_nonce(4);
        let now = Timestamp::from_micros(1_000_000);
        // Expires way beyond WITNESSABLE_EXPIRY_DURATION (50 minutes)
        let expires = Timestamp::from_micros(1_000_000 + 60 * 60 * 1_000_000); // +1 hour

        let result = db.witness_nonce(agent, nonce, now, expires).await.unwrap();
        assert_eq!(result, WitnessNonceResult::Future);
    }

    #[tokio::test]
    async fn nonce_already_seen() {
        let db = test_open_db(Conductor).await.unwrap();
        let agent = test_agent();
        let nonce = test_nonce(5);
        let now = Timestamp::from_micros(1_000_000);
        let expires = Timestamp::from_micros(2_000_000);

        let db_read: &DbRead<Conductor> = db.as_ref();

        // Not seen yet
        assert!(!db_read
            .nonce_already_seen(&agent, &nonce, now)
            .await
            .unwrap());

        // Witness it
        db.witness_nonce(agent.clone(), nonce, now, expires)
            .await
            .unwrap();

        // Now seen
        assert!(db_read
            .nonce_already_seen(&agent, &nonce, now)
            .await
            .unwrap());
    }

    // ========================================================================
    // Block operations
    // ========================================================================

    fn test_cell_id() -> CellId {
        CellId::new(
            DnaHash::from_raw_36(vec![0u8; 36]),
            AgentPubKey::from_raw_36(vec![1u8; 36]),
        )
    }

    fn test_block(start_us: i64, end_us: i64) -> Block {
        Block::new(
            BlockTarget::Cell(test_cell_id(), CellBlockReason::BadCrypto),
            InclusiveTimestampInterval::try_new(
                Timestamp::from_micros(start_us),
                Timestamp::from_micros(end_us),
            )
            .unwrap(),
        )
    }

    fn test_block_target_id() -> BlockTargetId {
        BlockTargetId::Cell(test_cell_id())
    }

    #[tokio::test]
    async fn block_and_is_blocked() {
        let db = test_open_db(Conductor).await.unwrap();
        let target = test_block_target_id();
        let mid = Timestamp::from_micros(500);

        // Not blocked initially
        assert!(!db.as_ref().is_blocked(target.clone(), mid).await.unwrap());

        // Block from 100..1000
        db.block(test_block(100, 1000)).await.unwrap();

        // Blocked within range
        assert!(db.as_ref().is_blocked(target.clone(), mid).await.unwrap());

        // Not blocked outside range
        assert!(!db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(1500))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn overlapping_blocks_accumulate_as_rows() {
        let db = test_open_db(Conductor).await.unwrap();
        let target = test_block_target_id();

        // Two overlapping blocks are recorded as independent rows; the target
        // is blocked across the union of their ranges.
        db.block(test_block(100, 500)).await.unwrap();
        db.block(test_block(400, 900)).await.unwrap();

        assert!(db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(200))
            .await
            .unwrap());
        assert!(db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(450))
            .await
            .unwrap());
        assert!(db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(700))
            .await
            .unwrap());
        assert!(!db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(50))
            .await
            .unwrap());
        assert!(!db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(950))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn blocks_with_different_reasons_are_independent() {
        let db = test_open_db(Conductor).await.unwrap();
        let target = test_block_target_id();

        let bad_crypto = Block::new(
            BlockTarget::Cell(test_cell_id(), CellBlockReason::BadCrypto),
            InclusiveTimestampInterval::try_new(
                Timestamp::from_micros(100),
                Timestamp::from_micros(500),
            )
            .unwrap(),
        );
        let invalid_op = Block::new(
            BlockTarget::Cell(
                test_cell_id(),
                CellBlockReason::InvalidOp(DhtOpHash::from_raw_36(vec![2u8; 36])),
            ),
            InclusiveTimestampInterval::try_new(
                Timestamp::from_micros(600),
                Timestamp::from_micros(900),
            )
            .unwrap(),
        );

        db.block(bad_crypto).await.unwrap();
        db.block(invalid_op).await.unwrap();

        // Both rows contribute to `is_blocked` on the shared target id.
        assert!(db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(200))
            .await
            .unwrap());
        assert!(db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(700))
            .await
            .unwrap());
        // Outside both ranges: not blocked.
        assert!(!db
            .as_ref()
            .is_blocked(target.clone(), Timestamp::from_micros(550))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn is_any_blocked() {
        let db = test_open_db(Conductor).await.unwrap();
        let ts = Timestamp::from_micros(500);

        // Empty vec returns false
        assert!(!db.as_ref().is_any_blocked(vec![], ts).await.unwrap());

        // Block cell 1
        db.block(test_block(100, 1000)).await.unwrap();

        let target = test_block_target_id();

        // Single blocked target
        assert!(db
            .as_ref()
            .is_any_blocked(vec![target.clone()], ts)
            .await
            .unwrap());

        // Mix of blocked and unblocked — any match is enough
        let other_cell = CellId::new(
            DnaHash::from_raw_36(vec![9u8; 36]),
            AgentPubKey::from_raw_36(vec![8u8; 36]),
        );
        let other_target = BlockTargetId::Cell(other_cell.clone());

        assert!(db
            .as_ref()
            .is_any_blocked(vec![target, other_target.clone()], ts)
            .await
            .unwrap());

        // Only unblocked target — returns false
        assert!(!db
            .as_ref()
            .is_any_blocked(vec![other_target], ts)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn tx_commit_persists() {
        let db = test_open_db(Conductor).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.set_conductor_tag("from-tx").await.unwrap();
        // Read-your-own-writes inside the transaction (reads on TxWrite go through as_mut).
        assert_eq!(
            tx.as_mut().get_conductor_tag().await.unwrap(),
            Some("from-tx".to_string())
        );
        tx.commit().await.unwrap();

        assert_eq!(
            db.as_ref().get_conductor_tag().await.unwrap(),
            Some("from-tx".to_string())
        );
    }

    #[tokio::test]
    async fn tx_read_only_snapshot() {
        // A read-only transaction from DbRead::begin(), exercising TxRead.
        let db = test_open_db(Conductor).await.unwrap();
        db.set_conductor_tag("initial").await.unwrap();

        let db_read: DbRead<Conductor> = db.clone().into();
        let mut tx = db_read.begin().await.unwrap();

        // Outside writes don't affect the snapshot-consistent transaction view.
        // (We don't strictly assert isolation semantics — WAL gives consistent
        // reads but sqlite's default DEFERRED mode means the snapshot starts
        // at first read. We just verify the read API works.)
        assert_eq!(
            tx.get_conductor_tag().await.unwrap(),
            Some("initial".to_string())
        );
        tx.close().await.unwrap();
    }

    #[tokio::test]
    async fn tx_rollback_discards() {
        let db = test_open_db(Conductor).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.set_conductor_tag("not-persisted").await.unwrap();
        tx.rollback().await.unwrap();

        assert_eq!(db.as_ref().get_conductor_tag().await.unwrap(), None);
    }

    #[tokio::test]
    async fn tx_drop_without_commit_rolls_back() {
        let db = test_open_db(Conductor).await.unwrap();

        {
            let mut tx = db.begin().await.unwrap();
            tx.set_conductor_tag("dropped").await.unwrap();
            // drop without commit
        }

        assert_eq!(db.as_ref().get_conductor_tag().await.unwrap(), None);
    }

    #[tokio::test]
    async fn tx_with_block_and_witness_nonce_can_be_rolled_back() {
        // Verifies multi-statement ops (block, witness_nonce) share the tx.
        let db = test_open_db(Conductor).await.unwrap();
        let agent = test_agent();
        let nonce = test_nonce(42);
        let now = Timestamp::from_micros(1_000_000);
        let expires = Timestamp::from_micros(2_000_000);

        let mut tx = db.begin().await.unwrap();
        tx.block(test_block(100, 1000)).await.unwrap();
        let witness = tx
            .witness_nonce(agent.clone(), nonce, now, expires)
            .await
            .unwrap();
        assert_eq!(witness, WitnessNonceResult::Fresh);
        tx.rollback().await.unwrap();

        // Neither the block nor the witnessed nonce survived the rollback.
        let target = test_block_target_id();
        assert!(!db
            .as_ref()
            .is_blocked(target, Timestamp::from_micros(500))
            .await
            .unwrap());
        assert!(!db
            .as_ref()
            .nonce_already_seen(&agent, &nonce, now)
            .await
            .unwrap());
    }
}
