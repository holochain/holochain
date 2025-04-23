//! Types for reporting storage information about the holochain conductor and hApps.

use holochain_types::prelude::*;

/// Storage info for DNA used by one or more hApps.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct DnaStorageInfo {
    /// The size of the authored data as reported by the database.
    pub authored_data_size: usize,
    /// The size of the authored data on disk.
    pub authored_data_size_on_disk: usize,
    /// The size of the DHT data as reported by the database.
    pub dht_data_size: usize,
    /// The size of the DHT data on disk.
    pub dht_data_size_on_disk: usize,
    /// The size of the cache data as reported by the database.
    pub cache_data_size: usize,
    /// The size of the cache data on disk.
    pub cache_data_size_on_disk: usize,
    /// The DNA hash that this storage info is for.
    pub dna_hash: DnaHash,
    /// Which apps use this DNA.
    pub used_by: Vec<InstalledAppId>,
}

/// The type of storage blob
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum StorageBlob {
    /// Storage blob used by hApps to store data
    Dna(DnaStorageInfo),
}

/// Response type for storage used by holochain and applications
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct StorageInfo {
    /// The details of blob storage used.
    pub blobs: Vec<StorageBlob>,
}
