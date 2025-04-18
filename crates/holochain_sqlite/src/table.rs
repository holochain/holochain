//! Functionality for safely accessing databases.

use rusqlite::Connection;

use crate::db::DbKind;

/// Enumeration of all databases needed by Holochain
pub(crate) fn initialize_database(conn: &mut Connection, db_kind: DbKind) -> rusqlite::Result<()> {
    match db_kind {
        DbKind::Dht(_) => {
            crate::schema::SCHEMA_CELL.initialize(conn, Some(db_kind))?;
        }
        DbKind::Authored(_) => {
            crate::schema::SCHEMA_CELL.initialize(conn, Some(db_kind))?;
        }
        DbKind::Conductor => {
            crate::schema::SCHEMA_CONDUCTOR.initialize(conn, Some(db_kind))?;
        }
        DbKind::Wasm => {
            crate::schema::SCHEMA_WASM.initialize(conn, Some(db_kind))?;
        }
        DbKind::Cache(_) => {
            crate::schema::SCHEMA_CELL.initialize(conn, Some(db_kind))?;
        }
        DbKind::PeerMetaStore(_) => {
            crate::schema::SCHEMA_PEER_META_STORE.initialize(conn, Some(db_kind))?;
        }
        #[cfg(feature = "test_utils")]
        DbKind::Test(_) => {
            // Nothing to do
        }
    }
    Ok(())
}
