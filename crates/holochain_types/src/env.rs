//! An "Env" Combines a database reference with a KeystoreSender

use std::path::Path;

use holochain_keystore::KeystoreSender;
use holochain_sqlite::prelude::*;
use shrinkwraprs::Shrinkwrap;

pub use holochain_sqlite::conn::DbSyncLevel;
/// Read access to a database, plus a keystore channel sender
#[derive(Clone, Shrinkwrap)]
pub struct EnvRead {
    #[shrinkwrap(main_field)]
    db: DbRead,
    keystore: KeystoreSender,
}

impl EnvRead {
    /// Accessor
    pub fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    /// Construct from components
    pub fn from_parts(db: DbRead, keystore: KeystoreSender) -> Self {
        Self { db, keystore }
    }
}

/// Write access to a database, plus a keystore channel sender
#[derive(Clone, Shrinkwrap)]
pub struct EnvWrite {
    #[shrinkwrap(main_field)]
    db: DbWrite,
    keystore: KeystoreSender,
}

impl EnvWrite {
    /// Constructor
    pub fn open(path: &Path, kind: DbKind, keystore: KeystoreSender) -> DatabaseResult<Self> {
        Self::open_with_sync_level(path, kind, keystore, DbSyncLevel::default())
    }

    /// Open a database with a set synchronous level.
    /// Note this won't override a database that already exists with a different level.
    pub fn open_with_sync_level(
        path: &Path,
        kind: DbKind,
        keystore: KeystoreSender,
        sync_level: DbSyncLevel,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            db: DbWrite::open_with_sync_level(path, kind, sync_level)?,
            keystore,
        })
    }

    /// Test constructor
    pub fn test(
        tmpdir: &tempdir::TempDir,
        kind: DbKind,
        keystore: KeystoreSender,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            db: DbWrite::test(tmpdir, kind)?,
            keystore,
        })
    }

    /// Kind Accessor
    pub fn kind(&self) -> &DbKind {
        self.db.kind()
    }

    /// Accessor
    pub fn keystore(&self) -> KeystoreSender {
        self.keystore.clone()
    }

    /// Remove the db and directory
    pub async fn remove(self) -> DatabaseResult<()> {
        self.db.remove().await
    }
}

impl From<EnvRead> for DbRead {
    fn from(env: EnvRead) -> DbRead {
        env.db
    }
}

impl From<EnvWrite> for DbWrite {
    fn from(env: EnvWrite) -> DbWrite {
        env.db
    }
}

impl From<EnvWrite> for DbRead {
    fn from(env: EnvWrite) -> DbRead {
        env.db.into()
    }
}

impl From<EnvWrite> for EnvRead {
    fn from(env: EnvWrite) -> EnvRead {
        Self {
            db: env.db.into(),
            keystore: env.keystore,
        }
    }
}

/// FIXME: this ain't right!! But we have had this in the code for a long time.
impl From<EnvRead> for EnvWrite {
    fn from(env: EnvRead) -> EnvWrite {
        Self {
            db: env.db.into(),
            keystore: env.keystore,
        }
    }
}
