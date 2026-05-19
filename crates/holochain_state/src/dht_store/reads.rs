//! Read operations on the per-DNA DHT store.
//!
//! Methods on [`DhtStoreRead`] expose domain-meaningful reads for the
//! holochain crate's workflows. They delegate to `holochain_data`'s
//! `DbRead<Dht>` primitives and return values in terms of the project's
//! existing domain types.

use super::{DhtStore, DhtStoreRead};

impl<Db> DhtStore<Db>
where
    Db: AsRef<holochain_data::DbRead<holochain_data::kind::Dht>>,
{
    // Read methods are added in the following tasks. Keep this module
    // separate so it stays focused on reads.
}

// Compile-only sanity check that the read-only alias resolves correctly.
#[allow(dead_code)]
fn _read_only_alias_compiles(_: DhtStoreRead) {}

#[cfg(test)]
mod tests {
    // Per-method tests are added alongside their read methods below.
}
