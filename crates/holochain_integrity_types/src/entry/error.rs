use super::*;

/// Errors involving app entry creation
#[derive(Debug, Clone, PartialEq)]
pub enum EntryError {
    /// The entry is too large to be created
    EntryTooLarge(usize),

    /// SerializedBytes passthrough
    SerializedBytes(SerializedBytesError),
}

impl std::error::Error for EntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EntryError::EntryTooLarge(_) => None,
            EntryError::SerializedBytes(e) => e.source(),
        }
    }
}

impl From<SerializedBytesError> for EntryError {
    fn from(e: SerializedBytesError) -> Self {
        Self::SerializedBytes(e)
    }
}

impl core::fmt::Display for EntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryError::EntryTooLarge(bytes)=> write!(
                f,
                "Attempted to create an Entry whose size exceeds the limit.\nEntry size: {}\nLimit: {}",
                bytes,
                MAX_ENTRY_SIZE
            ),
            EntryError::SerializedBytes(s) => s.fmt(f),
        }
    }
}
