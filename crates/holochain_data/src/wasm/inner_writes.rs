use sqlx::{Executor, Sqlite};

use crate::models::wasm::{CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel};

/// Store WASM bytecode.
pub(super) async fn put_wasm<'e, E>(executor: E, hash: &[u8], code: &[u8]) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT OR REPLACE INTO Wasm (hash, code) VALUES (?, ?)")
        .bind(hash)
        .bind(code)
        .execute(executor)
        .await?;

    Ok(())
}

/// Insert or replace a single DNA definition row.
pub(super) async fn insert_dna_def<'e, E>(executor: E, model: &DnaDefModel) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO DnaDef (hash, agent, name, network_seed, properties, lineage) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&model.hash)
    .bind(&model.agent)
    .bind(&model.name)
    .bind(&model.network_seed)
    .bind(&model.properties)
    .bind(&model.lineage)
    .execute(executor)
    .await?;

    Ok(())
}

/// Delete all integrity zome rows for a given DNA hash and agent.
pub(super) async fn delete_integrity_zomes<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM IntegrityZome WHERE dna_hash = ? AND agent = ?")
        .bind(dna_hash)
        .bind(agent)
        .execute(executor)
        .await?;

    Ok(())
}

/// Delete all coordinator zome rows for a given DNA hash and agent.
pub(super) async fn delete_coordinator_zomes<'e, E>(
    executor: E,
    dna_hash: &[u8],
    agent: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM CoordinatorZome WHERE dna_hash = ? AND agent = ?")
        .bind(dna_hash)
        .bind(agent)
        .execute(executor)
        .await?;

    Ok(())
}

/// Insert a single integrity zome row.
pub(super) async fn insert_integrity_zome<'e, E>(
    executor: E,
    model: &IntegrityZomeModel,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO IntegrityZome (dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&model.dna_hash)
    .bind(&model.agent)
    .bind(model.zome_index)
    .bind(&model.zome_name)
    .bind(model.wasm_hash.as_deref())
    .bind(&model.dependencies)
    .execute(executor)
    .await?;

    Ok(())
}

/// Insert a single coordinator zome row.
pub(super) async fn insert_coordinator_zome<'e, E>(
    executor: E,
    model: &CoordinatorZomeModel,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO CoordinatorZome (dna_hash, agent, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&model.dna_hash)
    .bind(&model.agent)
    .bind(model.zome_index)
    .bind(&model.zome_name)
    .bind(model.wasm_hash.as_deref())
    .bind(&model.dependencies)
    .execute(executor)
    .await?;

    Ok(())
}

/// Insert or replace a single entry definition row.
pub(super) async fn put_entry_def<'e, E>(executor: E, model: &EntryDefModel) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR REPLACE INTO EntryDef (key, entry_def_id, entry_def_id_type, visibility, required_validations) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&model.key)
    .bind(&model.entry_def_id)
    .bind(&model.entry_def_id_type)
    .bind(&model.visibility)
    .bind(model.required_validations)
    .execute(executor)
    .await?;

    Ok(())
}
