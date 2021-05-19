//! Functionality for safely accessing databases.

use rusqlite::Connection;

use crate::db::DbKind;

/// Enumeration of all databases needed by Holochain
pub(crate) fn initialize_database(conn: &mut Connection, db_kind: &DbKind) -> rusqlite::Result<()> {
    match db_kind {
        DbKind::Cell(_) => {
            crate::schema::SCHEMA_CELL.initialize(conn, Some(db_kind))?;
        }
        DbKind::Conductor => {
            crate::schema::SCHEMA_CONDUCTOR.initialize(conn, Some(db_kind))?;
        }
        DbKind::Wasm => {
            crate::schema::SCHEMA_WASM.initialize(conn, Some(db_kind))?;
        }
        DbKind::P2p(_) => {
            crate::schema::SCHEMA_P2P.initialize(conn, Some(db_kind))?;
        }
        DbKind::Cache(_) => {
            crate::schema::SCHEMA_CELL.initialize(conn, Some(db_kind))?;
        }
    }
    Ok(())
}
