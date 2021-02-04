use super::*;

/// Errors involving app entry creation
#[derive(Debug, Clone, thiserror::Error)]
pub enum EntryError {
    /// The entry is too large to be created
    #[error(
        "Attempted to create an Entry whose size exceeds the limit.\nEntry size: {0}\nLimit: {}",
        ENTRY_SIZE_LIMIT
    )]
    EntryTooLarge(usize),

    /// SerializedBytes passthrough
    #[error(transparent)]
    SerializedBytes(#[from] SerializedBytesError),
}
