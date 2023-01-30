//! Schema and migration definitions
//!
//! To create a new migration, add a new [`Migration`] object to the `migrations`
//! vec for a particular schema, and bump the `current_index` by 1.
//! The `Migration` must specify the actual forward migration script, as well as
//! an updated schema defining the result of running the migration.
//!
//! Currently, the updated schema only serves as a point of reference for examining
//! the current schema. In the future, we should find a way to compare the actual
//! schema resulting from migrations with the schema provided, to make sure they match.
//!
//! Note that there is code in `build.rs` which fails the build if any schema or migration
//! file has a change according to `git diff`. This will hopefully help prevent accidental
//! modification of schemas, which should never be committed.

use once_cell::sync::Lazy;
use rusqlite::Connection;

use crate::db::DbKind;

pub static SCHEMA_CELL: Lazy<Schema> = Lazy::new(|| Schema {
    current_index: 1,
    migrations: vec![
        M::initial(include_str!("sql/cell/schema/0.sql")),
        M {
            forward: include_str!("sql/cell/schema/1-up.sql").into(),
            schema: include_str!("sql/cell/schema/1.sql").into(),
        },
    ],
});

pub static SCHEMA_CONDUCTOR: Lazy<Schema> = Lazy::new(|| Schema {
    current_index: 0,
    migrations: vec![M::initial(include_str!("sql/conductor/schema/0.sql"))],
});

pub static SCHEMA_WASM: Lazy<Schema> = Lazy::new(|| Schema {
    current_index: 0,
    migrations: vec![M::initial(include_str!("sql/wasm/schema/0.sql"))],
});

pub static SCHEMA_P2P_STATE: Lazy<Schema> = Lazy::new(|| Schema {
    current_index: 0,
    migrations: vec![M::initial(include_str!("sql/p2p_agent_store/schema/0.sql"))],
});

pub static SCHEMA_P2P_METRICS: Lazy<Schema> = Lazy::new(|| Schema {
    current_index: 0,
    migrations: vec![M::initial(include_str!("sql/p2p_metrics/schema/0.sql"))],
});

pub struct Schema {
    current_index: usize,
    migrations: Vec<Migration>,
}

impl Schema {
    /// Determine if any database migrations need to run, and run them if so.
    /// The decision is based on the difference between this Schema's
    /// current_index and the user_version pragma value in the database itself.
    /// NB: The current_index is 0-based, and the user_version is 1-based.
    pub fn initialize(
        &self,
        conn: &mut Connection,
        db_kind: Option<DbKind>,
    ) -> rusqlite::Result<()> {
        let user_version: u16 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let db_kind = db_kind
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "<no name>".to_string());

        if user_version == 0 {
            // database just needs to be created / initialized
            self.migrations[self.current_index].initialize(conn)?;
            tracing::info!("database initialized: {}", db_kind);
            return Ok(());
        } else {
            let current_index = user_version as usize - 1;
            match current_index.cmp(&self.current_index) {
                std::cmp::Ordering::Less => {
                    // run forward migrations
                    for v in current_index..self.current_index + 1 {
                        self.migrations[v].run_forward(conn)?;
                        // set the DB user_version so that next time we don't run
                        // the same migration
                        conn.pragma_update(None, "user_version", v + 1)?;
                    }
                    tracing::info!(
                        "database forward migrated: {} from {} to {}",
                        db_kind,
                        current_index,
                        self.current_index
                    );
                }
                std::cmp::Ordering::Equal => {
                    tracing::debug!(
                        "database needed no migration or initialization, good to go: {}",
                        db_kind
                    );
                }
                std::cmp::Ordering::Greater => {
                    unimplemented!("backward migrations unimplemented");
                }
            }
        }

        Ok(())
    }
}

pub struct Migration {
    schema: Sql,
    forward: Sql,
}

impl Migration {
    pub fn initial(schema: &str) -> Self {
        Self {
            schema: schema.into(),
            forward: "".into(),
        }
    }

    pub fn initialize(&self, conn: &mut Connection) -> rusqlite::Result<()> {
        conn.execute_batch(&self.schema)?;
        Ok(())
    }

    pub fn run_forward(&self, conn: &mut Connection) -> rusqlite::Result<()> {
        conn.execute_batch(&self.forward)?;
        Ok(())
    }
}
type M = Migration;

type Sql = String;
