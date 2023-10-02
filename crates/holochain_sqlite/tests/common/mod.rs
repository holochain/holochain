use holochain_sqlite::prelude::{DbKind, DbKindT};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TestDatabaseKind {
    name: String,
}

impl TestDatabaseKind {
    pub fn new() -> Self {
        Self {
            name: nanoid::nanoid!(),
        }
    }
}

impl DbKindT for TestDatabaseKind {
    fn kind(&self) -> DbKind {
        // The code stores by kind so this needs to be unique per test
        DbKind::Test(self.name.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        PathBuf::from(self.name.as_str())
    }

    fn if_corrupt_wipe(&self) -> bool {
        false
    }
}
