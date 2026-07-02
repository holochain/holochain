//! `DbRead<Dht>` API for database size statistics.

use super::super::inner::db_size;
use crate::handles::DbRead;
use crate::kind::Dht;

impl DbRead<Dht> {
    /// Total bytes occupied on disk by every page of the database, including
    /// the unused (free) bytes within each page.
    pub async fn get_size_on_disk(&self) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        db_size::get_size_on_disk(&mut *conn).await
    }

    /// Bytes actually in use by the database, excluding the free space within
    /// pages.
    pub async fn get_used_size(&self) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        db_size::get_used_size(&mut *conn).await
    }
}
