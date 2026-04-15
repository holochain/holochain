//! Examples demonstrating compile-time checked sqlx query patterns.
//!
//! These examples exercise the `query!` and `query_as!` macros against both
//! the wasm and conductor schemas, ensuring `sqlx prepare` keeps the offline
//! query cache up to date.

use sqlx::FromRow;

// --- Wasm schema examples ---

/// Example struct mapped from the Wasm table.
#[derive(Debug, Clone, FromRow)]
pub struct WasmRow {
    pub hash: Vec<u8>,
    pub code: Vec<u8>,
}

/// Compile-time checked insert into the Wasm table.
pub async fn insert_wasm(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    hash: &[u8],
    code: &[u8],
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT OR IGNORE INTO Wasm (hash, code) VALUES (?, ?)",
        hash,
        code
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Compile-time checked select from the Wasm table.
pub async fn get_wasm(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    hash: &[u8],
) -> sqlx::Result<Option<WasmRow>> {
    let row = sqlx::query_as!(
        WasmRow,
        r#"SELECT hash as "hash!", code as "code!" FROM Wasm WHERE hash = ?"#,
        hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// --- Conductor schema examples ---

/// Compile-time checked read of the conductor tag.
pub async fn get_conductor_tag(pool: &sqlx::Pool<sqlx::Sqlite>) -> sqlx::Result<Option<String>> {
    let row = sqlx::query!("SELECT tag FROM Conductor WHERE id = 1")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.tag))
}

/// Compile-time checked upsert of the conductor tag.
pub async fn set_conductor_tag(pool: &sqlx::Pool<sqlx::Sqlite>, tag: &str) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO Conductor (id, tag) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET tag = excluded.tag",
        tag
    )
    .execute(pool)
    .await?;
    Ok(())
}
