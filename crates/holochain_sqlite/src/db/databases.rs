use crate::db::access::DbWrite;
use crate::db::kind::DbKindT;
use crate::prelude::*;
use once_cell::sync::Lazy;
use std::{
    any::Any,
    collections::HashMap,
    path::{Path, PathBuf},
};

/// A map over any database type key'd by the full path to the database.
pub(super) struct Databases {
    dbs: parking_lot::RwLock<HashMap<PathBuf, Box<dyn Any + Send + Sync>>>,
}

pub(super) static DATABASE_HANDLES: Lazy<Databases> = Lazy::new(|| {
    // This is just a convenient place that we know gets initialized
    // both in the final binary holochain && in all relevant tests
    //
    // Holochain (and most binaries) are left in invalid states
    // if a thread panic!s - switch to failing fast in that case.
    //
    // We tried putting `panic = "abort"` in the Cargo.toml,
    // but somehow that breaks the wasmer / test_utils integration.

    let orig_handler = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // print the panic message
        eprintln!("FATAL PANIC {:#?}", panic_info);
        // invoke the original handler
        orig_handler(panic_info);
        // // Abort the process
        // // TODO - we need a better solution than this, but if there is
        // // no better solution, we can uncomment the following line:
        // std::process::abort();
    }));

    Databases::new()
});

impl Databases {
    /// Create a new database map.
    pub(super) fn new() -> Self {
        Databases {
            dbs: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Get a database if it exists or
    /// create it.
    pub(super) fn get_or_insert<Kind, F>(
        &self,
        kind: &Kind,
        path_prefix: &Path,
        insert: F,
    ) -> DatabaseResult<DbWrite<Kind>>
    where
        Kind: DbKindT + Send + Sync + 'static,
        F: FnOnce(Kind) -> DatabaseResult<DbWrite<Kind>>,
    {
        // Create the full path from the prefix and the kind.
        let path = path_prefix.join(kind.filename());

        // First try a quick read lock because for the majority of calls
        // the database will already exist.
        let ret = self
            .dbs
            .read()
            .get(&path)
            .and_then(|d| d.downcast_ref::<DbWrite<Kind>>().cloned());
        match ret {
            Some(ret) => Ok(ret),
            None => match self.dbs.write().entry(path) {
                // Note that this downcast is safe because the path is created
                // from the kind so will always be the correct type.
                std::collections::hash_map::Entry::Occupied(o) => Ok(o
                    .get()
                    .downcast_ref::<DbWrite<Kind>>()
                    .expect("Downcast to db kind failed. This is a bug")
                    .clone()),
                std::collections::hash_map::Entry::Vacant(v) => {
                    // If the db is missing we run the closure to create it.
                    // Note the `Kind` is enforced by the closure return type.
                    let db = insert(kind.clone())?;
                    v.insert(Box::new(db.clone()));
                    Ok(db)
                }
            },
        }
    }
}
