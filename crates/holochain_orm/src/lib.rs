//! A wrapper around SeaORM, configured for use in Holochain.
//!
//! This crate does not implement an ORM itself but provides what Holochain needs to use SeaORM.

use std::path::PathBuf;
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
    path: PathBuf,
    database_id: I,
) -> Result<HolochainDbConn<I>, DbErr> {
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

    // Configure connection pool similar to holochain_sqlite:
    // - Max connections: num_read_threads * 2 + 1 (simplified to a reasonable default)
    // - Min idle: 0 (close idle connections)
    // - Idle timeout: 30 seconds
    opt.max_connections(20)
        .min_connections(0)
        .idle_timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(30));
    
    Database::connect(opt).await
}
