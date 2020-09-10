//! Types for source chain queries

use crate::header::{EntryType, Header, HeaderType};
pub use holochain_serialized_bytes::prelude::*;

/// Query arguments
#[derive(
    serde::Serialize, serde::Deserialize, SerializedBytes, Default, PartialEq, Clone, Debug,
)]
#[non_exhaustive]
pub struct ChainQueryFilter {
    /// The range of source chain sequence numbers to match.
    /// Inclusive start, exclusive end.
    pub sequence_range: Option<std::ops::Range<u32>>,
    /// Filter by EntryType
    pub entry_type: Option<EntryType>,
    /// Filter by HeaderType
    pub header_type: Option<HeaderType>,
}

impl ChainQueryFilter {
    /// Create a no-op ChainQueryFilter which returns everything
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

    /// Perform the boolean check which this filter represents
    pub fn check(&self, header: &Header) -> bool {
        let check_range = self
            .sequence_range
            .as_ref()
            .map(|range| range.contains(&header.header_seq()))
            .unwrap_or(true);
        let check_header_type = self
            .header_type
            .as_ref()
            .map(|header_type| header.header_type() == *header_type)
            .unwrap_or(true);
        let check_entry_type = self
            .entry_type
            .as_ref()
            .map(|entry_type| {
                header
                    .entry_type()
                    .map(|header_entry_type| *header_entry_type == *entry_type)
                    .unwrap_or(true)
            })
            .unwrap_or(true);
        check_range && check_header_type && check_entry_type
    }
}

#[cfg(test)]
#[cfg(feature = "fixturators")]
mod tests {
    use crate::header::{builder, EntryType, HeaderBuilderCommon};
    use crate::{fixt::AppEntryTypeFixturator, link::LinkTag};
    use crate::{fixt::*, Header};
    use ::fixt::prelude::*;
    use builder::HeaderBuilder;

    use super::ChainQueryFilter;

    /// Create three Headers with various properties.
    /// Also return the EntryTypes used to construct the first two headers.
    fn fixtures() -> ([Header; 3], (EntryType, EntryType)) {
        let author = fixt!(AgentPubKey);
        let entry_type_1 = EntryType::App(fixt!(AppEntryType));
        let entry_type_2 = EntryType::AgentPubKey;

        let header_1 = builder::EntryCreate {
            entry_type: entry_type_1.clone(),
            entry_hash: fixt!(EntryHash),
        }
        .build(HeaderBuilderCommon::new(
            author.clone(),
            fixt!(Timestamp),
            3,
            fixt!(HeaderHash),
        ))
        .into();

        let header_2 = builder::EntryUpdate {
            entry_type: entry_type_2.clone(),
            entry_hash: fixt!(EntryHash),
            original_entry_address: fixt!(EntryHash),
            original_header_address: fixt!(HeaderHash),
        }
        .build(HeaderBuilderCommon::new(
            author.clone(),
            fixt!(Timestamp),
            5,
            fixt!(HeaderHash),
        ))
        .into();

        let header_3 = builder::LinkAdd {
            base_address: fixt!(EntryHash),
            target_address: fixt!(EntryHash),
            zome_id: 0.into(),
            tag: LinkTag::new([0]),
        }
        .build(HeaderBuilderCommon::new(
            author.clone(),
            fixt!(Timestamp),
            5,
            fixt!(HeaderHash),
        ))
        .into();

        ([header_1, header_2, header_3], (entry_type_1, entry_type_2))
    }

    fn map_query(query: &ChainQueryFilter, headers: &[Header]) -> Vec<bool> {
        headers.iter().map(|h| query.check(h)).collect::<Vec<_>>()
    }

    #[test]
    fn filter_by_entry_type() {
        let (headers, (entry_type_1, entry_type_2)) = fixtures();

        let query_1 = ChainQueryFilter::new().entry_type(entry_type_1);
        let query_2 = ChainQueryFilter::new().entry_type(entry_type_2);

        assert_eq!(map_query(&query_1, &headers), [true, false, true].to_vec(),);
        assert_eq!(map_query(&query_2, &headers), [false, true, true].to_vec(),);
    }

    #[test]
    fn filter_by_header_type() {
        todo!()
    }

    #[test]
    fn filter_by_chain_sequence() {
        todo!()
    }
}
