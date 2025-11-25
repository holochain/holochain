//! A wrapper around SeaORM, configured for use in Holochain.
//!
//! This crate does not implement an ORM itself but provides what Holochain needs to use SeaORM.

use std::path::PathBuf;
use sea_orm::DatabaseConnection;

pub trait DatabaseIdentifier {
    fn database_id(&self) -> &str;
}

pub struct HolochainDbConn<I: DatabaseIdentifier> {
    pub conn: DatabaseConnection,
    pub identifier: I,
}

fn setup_holochain_orm<I: DatabaseIdentifier>(path: PathBuf, database_id: I) -> HolochainDbConn<I> {
    let db: DatabaseConnection = unimplemented!();
}

#[cfg(feature = "test-utils")]
fn test_setup_holochain_orm() {

}
