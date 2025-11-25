//! A wrapper around SeaORM, configured for use in Holochain.
//!
//! This crate does not implement an ORM itself but provides what Holochain needs to use SeaORM.

use std::path::Path;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr, RuntimeErr};

pub trait DatabaseIdentifier {
    fn database_id(&self) -> &str;
}

pub struct HolochainDbConn<I: DatabaseIdentifier> {
    pub conn: DatabaseConnection,
    pub identifier: I,
}

/// Open a database connection at the given directory path.
/// The database file name is constructed from the `database_id`.
///
/// # Errors
/// Returns an error if `path` is not a directory.
pub async fn setup_holochain_orm<I: DatabaseIdentifier>(
    path: impl AsRef<Path>,
    database_id: I,
) -> Result<HolochainDbConn<I>, DbErr> {
    let path = path.as_ref();
    if !path.is_dir() {
        return Err(DbErr::Conn(RuntimeErr::Internal(
            format!("Path must be a directory: {}", path.display())
        )));
    }
    
    let db_file = path.join(database_id.database_id());
    let connection_string = format!("sqlite://{}?mode=rwc", db_file.display());
    let conn = connect_database(&connection_string).await?;
    Ok(HolochainDbConn {
        conn,
        identifier: database_id,
    })
}

#[cfg(feature = "test-utils")]
pub async fn test_setup_holochain_orm<I: DatabaseIdentifier>(
    database_id: I,
) -> Result<HolochainDbConn<I>, DbErr> {
    let connection_string = "sqlite::memory:".to_string();
    let conn = connect_database(&connection_string).await?;
    Ok(HolochainDbConn {
        conn,
        identifier: database_id,
    })
}

/// Connect to a SQLite database using the provided connection string.
async fn connect_database(connection_string: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(connection_string);

    // Configure connection pool:
    // SeaORM handles read/write connections internally, so we just need
    // a reasonable pool size based on CPU count.
    let max_cons = num_read_threads();
    
    opt.max_connections(max_cons as u32)
        .min_connections(0)
        .idle_timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(30));
    
    Database::connect(opt).await
}

/// Calculate the number of read threads based on CPU count.
/// Returns at least 4, or the number of CPUs.
fn num_read_threads() -> usize {
    let num_cpus = num_cpus::get();
    std::cmp::max(num_cpus, 4)
}
