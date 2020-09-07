//! Types for source chain queries

use crate::header::{EntryType, HeaderType};

pub use holochain_serialized_bytes::prelude::*;

/// Query arguments
#[derive(
    serde::Serialize, serde::Deserialize, SerializedBytes, Default, PartialEq, Clone, Debug,
)]
#[non_exhaustive]
pub struct ChainQuery {
    /// The range of source chain sequence numbers to match.
    /// Inclusive start, exclusive end.
    pub sequence_range: Option<std::ops::Range<u32>>,
    /// Filter by EntryType
    pub entry_type: Option<EntryType>,
    /// Filter by HeaderType
    pub header_type: Option<HeaderType>,
}

impl ChainQuery {
    /// Create a no-op ChainQuery which returns everything
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter on sequence range
    pub fn sequence_range(mut self, sequence_range: std::ops::Range<u32>) -> Self {
        self.sequence_range = Some(sequence_range);
        self
    }

    /// Filter on entry type
    pub fn entry_type(mut self, entry_type: EntryType) -> Self {
        self.entry_type = Some(entry_type);
        self
    }

    /// Filter on header type
    pub fn header_type(mut self, header_type: HeaderType) -> Self {
        self.header_type = Some(header_type);
        self
    }
}
