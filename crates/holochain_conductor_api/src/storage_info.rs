use holochain_types::prelude::*;

/// Storage info for DNA used by one or more hApps.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct DnaStorageInfo {
    pub authored_data_size: usize,
    pub authored_data_size_on_disk: usize,
    pub dht_data_size: usize,
    pub dht_data_size_on_disk: usize,
    pub cache_data_size: usize,
    pub cache_data_size_on_disk: usize,
    pub used_by: Vec<InstalledAppId>,
}

/// The type of storage blob
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub enum StorageBlob {
    /// Storage blob used by hApps to store data
    Dna(DnaStorageInfo),
}

/// Response type for storage used by holochain and applications
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct StorageInfo {
    pub blobs: Vec<StorageBlob>,
}
