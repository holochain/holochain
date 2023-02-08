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
use rusqlite::{Connection, Transaction};

use crate::db::DbKind;

pub static SCHEMA_CELL: Lazy<Schema> = Lazy::new(|| Schema {
    migrations: vec![
        M::initial(include_str!("sql/cell/schema/0.sql")),
        M {
            forward: include_str!("sql/cell/schema/1-up.sql").into(),
            _schema: include_str!("sql/cell/schema/1.sql").into(),
        },
    ],
});

pub static SCHEMA_CONDUCTOR: Lazy<Schema> = Lazy::new(|| Schema {
    migrations: vec![
        M::initial(include_str!("sql/conductor/schema/0.sql")),
        // Everything in schema 0 has IF NOT EXISTS on it so we can rerun it.
        M {
            forward: include_str!("sql/conductor/schema/0.sql").into(),
            _schema: "".into(),
        },
    ],
});

pub static SCHEMA_WASM: Lazy<Schema> = Lazy::new(|| Schema {
    migrations: vec![M::initial(include_str!("sql/wasm/schema/0.sql"))],
});

pub static SCHEMA_P2P_STATE: Lazy<Schema> = Lazy::new(|| Schema {
    migrations: vec![M::initial(include_str!("sql/p2p_agent_store/schema/0.sql"))],
});

pub static SCHEMA_P2P_METRICS: Lazy<Schema> = Lazy::new(|| Schema {
    migrations: vec![M::initial(include_str!("sql/p2p_metrics/schema/0.sql"))],
});

pub struct Schema {
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

        let migrations_applied = user_version as usize;
        let num_migrations = self.migrations.len();
        match migrations_applied.cmp(&(num_migrations)) {
            std::cmp::Ordering::Less => {
                let mut txn = conn.transaction()?;
                // run forward migrations
                for v in migrations_applied..num_migrations {
                    self.migrations[v].run_forward(&mut txn)?;
                    // set the DB user_version so that next time we don't run
                    // the same migration
                    txn.pragma_update(None, "user_version", v + 1)?;
                }
                txn.commit()?;
                tracing::info!(
                    "database forward migrated: {} from {} to {}",
                    db_kind,
                    migrations_applied,
                    num_migrations - 1,
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

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Migration {
    _schema: Sql,
    forward: Sql,
}

impl Migration {
    /// The initial migration's forward migration is the entire schema
    pub fn initial(schema: &str) -> Self {
        Self {
            _schema: schema.into(),
            forward: schema.into(),
        }
    }

    pub fn run_forward(&self, txn: &mut Transaction) -> rusqlite::Result<()> {
        txn.execute_batch(&self.forward)?;
        Ok(())
    }
}
type M = Migration;

type Sql = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_initial() {
        let schema = Schema {
            migrations: vec![
                M::initial("CREATE TABLE Numbers (num INTEGER);"),
                M {
                    forward: "CREATE TABLE Names (name TEXT);".into(),
                    _schema: "n/a".into(),
                },
            ],
        };

        let mut conn = Connection::open_in_memory().unwrap();

        // The Names table doesn't exist yet, since the current_index is set to 0.
        schema.initialize(&mut conn, None).unwrap();
        assert_eq!(
            conn.execute("INSERT INTO Numbers (num) VALUES (1)", ())
                .unwrap(),
            1
        );
        assert_eq!(
            conn.execute("INSERT INTO Names (name) VALUES ('Mike')", ())
                .unwrap(),
            1
        );
    }

    #[test]
    fn test_migrations_sequential() {
        let mut schema = Schema {
            migrations: vec![M::initial("CREATE TABLE Numbers (num INTEGER);")],
        };

        let mut conn = Connection::open_in_memory().unwrap();

        // The Names table doesn't exist yet, since the current_index is set to 0.
        schema.initialize(&mut conn, None).unwrap();
        assert_eq!(
            conn.execute("INSERT INTO Numbers (num) VALUES (1)", ())
                .unwrap(),
            1
        );
        assert!(conn
            .execute("INSERT INTO Names (name) VALUES ('Mike')", ())
            .is_err());

        // This initialization will run only the second migration and create the Names table.
        schema.migrations = vec![
            M::initial("This bad SQL won't run, phew!"),
            M {
                forward: "CREATE TABLE Names (name TEXT);".into(),
                _schema: "n/a".into(),
            },
        ];
        schema.initialize(&mut conn, None).unwrap();
        assert_eq!(
            conn.execute("INSERT INTO Numbers (num) VALUES (1)", ())
                .unwrap(),
            1
        );
        assert_eq!(
            conn.execute("INSERT INTO Names (name) VALUES ('Mike')", ())
                .unwrap(),
            1
        );
    }
}
