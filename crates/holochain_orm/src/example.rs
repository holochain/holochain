//! Example demonstrating sqlx query patterns for mapping Rust types to database.

use sqlx::{FromRow, Row};
use crate::HolochainDbConn;

/// Example struct representing a row in the sample_data table.
///
/// This uses the `FromRow` derive to automatically map database columns to struct fields.
#[derive(Debug, Clone, FromRow)]
pub struct SampleData {
    pub id: i64,
    pub name: String,
    pub value: Option<String>,
    pub created_at: i64,
}

/// Example of inserting data using sqlx::query with bind parameters.
pub async fn insert_sample_data(
    conn: &HolochainDbConn<impl crate::DatabaseIdentifier>,
    name: &str,
    value: Option<&str>,
) -> sqlx::Result<i64> {
    let result = sqlx::query(
        "INSERT INTO sample_data (name, value) VALUES (?, ?)"
    )
    .bind(name)
    .bind(value)
    .execute(&conn.pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Example of selecting data using sqlx::query_as with automatic struct mapping.
pub async fn get_sample_data_by_id(
    conn: &HolochainDbConn<impl crate::DatabaseIdentifier>,
    id: i64,
) -> sqlx::Result<Option<SampleData>> {
    let result = sqlx::query_as::<_, SampleData>(
        "SELECT id, name, value, created_at FROM sample_data WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&conn.pool)
    .await?;

    Ok(result)
}

/// Example of selecting multiple rows.
pub async fn get_all_sample_data(
    conn: &HolochainDbConn<impl crate::DatabaseIdentifier>,
) -> sqlx::Result<Vec<SampleData>> {
    let results = sqlx::query_as::<_, SampleData>(
        "SELECT id, name, value, created_at FROM sample_data ORDER BY created_at DESC"
    )
    .fetch_all(&conn.pool)
    .await?;

    Ok(results)
}

/// Example of manual row mapping (useful when you need fine control).
pub async fn get_sample_data_manual(
    conn: &HolochainDbConn<impl crate::DatabaseIdentifier>,
    id: i64,
) -> sqlx::Result<Option<SampleData>> {
    let row = sqlx::query(
        "SELECT id, name, value, created_at FROM sample_data WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&conn.pool)
    .await?;

    Ok(row.map(|r| SampleData {
        id: r.get(0),
        name: r.get(1),
        value: r.get(2),
        created_at: r.get(3),
    }))
}

/// Example of updating data.
pub async fn update_sample_data(
    conn: &HolochainDbConn<impl crate::DatabaseIdentifier>,
    id: i64,
    new_value: &str,
) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE sample_data SET value = ? WHERE id = ?"
    )
    .bind(new_value)
    .bind(id)
    .execute(&conn.pool)
    .await?;

    Ok(result.rows_affected())
}

/// Example of deleting data.
pub async fn delete_sample_data(
    conn: &HolochainDbConn<impl crate::DatabaseIdentifier>,
    id: i64,
) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "DELETE FROM sample_data WHERE id = ?"
    )
    .bind(id)
    .execute(&conn.pool)
    .await?;

    Ok(result.rows_affected())
}
