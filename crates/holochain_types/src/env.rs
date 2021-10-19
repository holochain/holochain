//! An "Env" Combines a database reference with a KeystoreSender
//!

pub use holochain_sqlite::conn::DbSyncLevel;
pub use holochain_sqlite::db::*;

// use std::path::Path;

// use holochain_keystore::MetaLairClient;
// use holochain_sqlite::prelude::*;
// use shrinkwraprs::Shrinkwrap;

// /// Read access to a database, plus a keystore channel sender
// #[derive(Clone, Shrinkwrap)]
// pub struct DbReadOnly {
//     #[shrinkwrap(main_field)]
//     db: DbReadOnly,
//     keystore: MetaLairClient,
// }

// impl DbReadOnly {
//     /// Accessor
//     pub fn keystore(&self) -> &MetaLairClient {
//         &self.keystore
//     }

//     /// Construct from components
//     pub fn from_parts(db: DbReadOnly, keystore: MetaLairClient) -> Self {
//         Self { db, keystore }
//     }
// }

// /// Write access to a database, plus a keystore channel sender
// #[derive(Clone, Shrinkwrap)]
// pub struct DbWrite {
//     #[shrinkwrap(main_field)]
//     db: DbWrite,
//     keystore: MetaLairClient,
// }

// impl DbWrite {
//     /// Constructor
//     pub fn open(path: &Path, kind: DbKind, keystore: MetaLairClient) -> DatabaseResult<Self> {
//         Self::open_with_sync_level(path, kind, keystore, DbSyncLevel::default())
//     }

//     /// Open a database with a set synchronous level.
//     /// Note this won't override a database that already exists with a different level.
//     pub fn open_with_sync_level(
//         path: &Path,
//         kind: DbKind,
//         keystore: MetaLairClient,
//         sync_level: DbSyncLevel,
//     ) -> DatabaseResult<Self> {
//         Ok(Self {
//             db: DbWrite::open_with_sync_level(path, kind, sync_level)?,
//             keystore,
//         })
//     }

//     /// Test constructor
//     pub fn test(
//         tmpdir: &tempdir::TempDir,
//         kind: DbKind,
//         keystore: MetaLairClient,
//     ) -> DatabaseResult<Self> {
//         Ok(Self {
//             db: DbWrite::test(tmpdir, kind)?,
//             keystore,
//         })
//     }

//     /// Kind Accessor
//     pub fn kind(&self) -> &DbKind {
//         self.db.kind()
//     }

//     /// Accessor
//     pub fn keystore(&self) -> MetaLairClient {
//         self.keystore.clone()
//     }

//     /// Remove the db and directory
//     pub async fn remove(self) -> DatabaseResult<()> {
//         self.db.remove().await
//     }
// }

// impl From<DbReadOnly> for DbReadOnly {
//     fn from(env: DbReadOnly) -> DbReadOnly {
//         env.db
//     }
// }

// impl From<DbWrite> for DbWrite {
//     fn from(env: DbWrite) -> DbWrite {
//         env.db
//     }
// }

// impl From<DbWrite> for DbReadOnly {
//     fn from(env: DbWrite) -> DbReadOnly {
//         env.db.into()
//     }
// }

// impl From<DbWrite> for DbReadOnly {
//     fn from(env: DbWrite) -> DbReadOnly {
//         Self {
//             db: env.db.into(),
//             keystore: env.keystore,
//         }
//     }
// }

// /// FIXME: this ain't right!! But we have had this in the code for a long time.
// impl From<DbReadOnly> for DbWrite {
//     fn from(env: DbReadOnly) -> DbWrite {
//         Self {
//             db: env.db.into(),
//             keystore: env.keystore,
//         }
//     }
// }
