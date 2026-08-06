use holochain_types::prelude::*;

/// Storage info for DNA used by one or more hApps.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "api/admin/types.ts"))]
pub struct DnaStorageInfo {
    /// Size in bytes of the data in use by the DhtStore.
    pub dht_data_size: usize,
    /// Size in bytes on disk of the DhtStore, including free space reserved by
    /// the database.
    pub dht_data_size_on_disk: usize,
    /// The hash of the DNA this storage information is for.
    pub dna_hash: DnaHash,
    /// The installed apps that make use of this DNA.
    pub used_by: Vec<InstalledAppId>,
}

/// The type of storage blob
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "api/admin/types.ts"))]
pub enum StorageBlob {
    /// Storage blob used by hApps to store data
    Dna(DnaStorageInfo),
}

/// Response type for storage used by holochain and applications
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "api/admin/types.ts"))]
pub struct StorageInfo {
    /// The storage blobs that make up this storage report.
    pub blobs: Vec<StorageBlob>,
}
