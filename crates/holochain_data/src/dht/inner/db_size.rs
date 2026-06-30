//! Database size statistics derived from the `dbstat` virtual table.
//!
//! These mirror the legacy `holochain_sqlite::stats` helpers so that the
//! merged DHT store can report per-DNA storage usage. They rely on the
//! `dbstat` virtual table, which is compiled into the bundled SQLCipher build
//! (`SQLITE_ENABLE_DBSTAT_VTAB`).

use sqlx::{Executor, Sqlite};

/// Total bytes occupied on disk by every page of the database, including the
/// unused (free) bytes within each page.
///
/// Computed as `sum(pgsize)` over the `dbstat` virtual table.
pub(crate) async fn get_size_on_disk<'e, E>(executor: E) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let size: i64 = sqlx::query_scalar("SELECT COALESCE(sum(pgsize), 0) FROM dbstat")
        .fetch_one(executor)
        .await?;
    Ok(size as u64)
}

/// Bytes actually in use by the database, excluding the free space within
/// pages.
///
/// Computed as `sum(pgsize - unused)` over the `dbstat` virtual table.
pub(crate) async fn get_used_size<'e, E>(executor: E) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let size: i64 = sqlx::query_scalar("SELECT COALESCE(sum(pgsize - unused), 0) FROM dbstat")
        .fetch_one(executor)
        .await?;
    Ok(size as u64)
}

#[cfg(test)]
mod tests {
    use crate::kind::Dht;
    use crate::test_open_db;
    use holo_hash::DnaHash;
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    #[tokio::test]
    async fn dbstat_reports_nonzero_sizes() {
        let db = test_open_db(dht_id()).await.unwrap();

        // A migrated database has tables and indexes, so both measures must be
        // positive. This also confirms the `dbstat` virtual table is available
        // on this SQLite build.
        let on_disk = db.as_ref().get_size_on_disk().await.unwrap();
        let used = db.as_ref().get_used_size().await.unwrap();

        assert!(on_disk > 0, "expected nonzero on-disk size");
        assert!(used > 0, "expected nonzero used size");
        // Used size can never exceed the on-disk page total.
        assert!(used <= on_disk);
    }
}
