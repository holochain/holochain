use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbCacheError {
    #[error("Tried to integrate activity {1} after {0} but agent activity must be in order")]
    ActivityOutOfOrder(u32, u32),
    #[error("Database error: {0}")]
    DatabaseError(#[from] holochain_sqlite::prelude::DatabaseError),
}

pub type DbCacheResult<T> = Result<T, DbCacheError>;
