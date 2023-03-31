use holochain_types::prelude::*;

/// Storage blob used by hApps to store data.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AppDataStorageBlob {
    pub authored_data_size: usize,
    pub authored_data_size_on_disk: usize,
    pub dht_data_size: usize,
    pub dht_data_size_on_disk: usize,
    pub cache_data_size: usize,
    pub cache_data_size_on_disk: usize,
    pub used_by: Vec<InstalledAppId>,
}

/// The type of storage type blob
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub enum StorageBlob {
    /// Storage blob used by hApps to store data
    AppData(AppDataStorageBlob),
}

/// Response type for storage used by holochain and applications
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct StorageInfo {
    pub blobs: Vec<StorageBlob>,
}
