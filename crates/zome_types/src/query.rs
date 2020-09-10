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
    // TODO: can we generalize this over RangeBounds to allow unbounded ranges?
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
    use crate::fixt::AppEntryTypeFixturator;
    use crate::header::{EntryType, HeaderType};
    use crate::{fixt::*, Header};
    use ::fixt::prelude::*;

    use super::ChainQueryFilter;

    /// Create three Headers with various properties.
    /// Also return the EntryTypes used to construct the first two headers.
    fn fixtures() -> ([Header; 6], (EntryType, EntryType)) {
        let entry_type_1 = EntryType::App(fixt!(AppEntryType));
        let entry_type_2 = EntryType::AgentPubKey;

        let mut h1 = fixt!(EntryCreate);
        h1.entry_type = entry_type_1.clone();
        h1.header_seq = 0;

        let mut h2 = fixt!(EntryUpdate);
        h2.entry_type = entry_type_2.clone();
        h2.header_seq = 1;

        let mut h3 = fixt!(LinkAdd);
        h3.header_seq = 2;

        let mut h4 = fixt!(EntryCreate);
        h4.entry_type = entry_type_2.clone();
        h4.header_seq = 3;

        let mut h5 = fixt!(EntryUpdate);
        h5.entry_type = entry_type_1.clone();
        h5.header_seq = 4;

        let mut h6 = fixt!(LinkAdd);
        h6.header_seq = 5;

        let headers = [
            h1.into(),
            h2.into(),
            h3.into(),
            h4.into(),
            h5.into(),
            h6.into(),
        ];
        (headers, (entry_type_1, entry_type_2))
    }

    fn map_query(query: &ChainQueryFilter, headers: &[Header]) -> Vec<bool> {
        headers.iter().map(|h| query.check(h)).collect::<Vec<_>>()
    }

    #[test]
    fn filter_by_entry_type() {
        let (headers, (entry_type_1, entry_type_2)) = fixtures();

        let query_1 = ChainQueryFilter::new().entry_type(entry_type_1);
        let query_2 = ChainQueryFilter::new().entry_type(entry_type_2);

        assert_eq!(
            map_query(&query_1, &headers),
            [true, false, true, false, true, true].to_vec()
        );
        assert_eq!(
            map_query(&query_2, &headers),
            [false, true, true, true, false, true].to_vec()
        );
    }

    #[test]
    fn filter_by_header_type() {
        let (headers, _) = fixtures();

        let query_1 = ChainQueryFilter::new().header_type(HeaderType::EntryCreate);
        let query_2 = ChainQueryFilter::new().header_type(HeaderType::EntryUpdate);
        let query_3 = ChainQueryFilter::new().header_type(HeaderType::LinkAdd);

        assert_eq!(
            map_query(&query_1, &headers),
            [true, false, false, true, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_2, &headers),
            [false, true, false, false, true, false].to_vec()
        );
        assert_eq!(
            map_query(&query_3, &headers),
            [false, false, true, false, false, true].to_vec()
        );
    }

    #[test]
    fn filter_by_chain_sequence() {
        let (headers, _) = fixtures();

        let query_1 = ChainQueryFilter::new().sequence_range(0..1);
        let query_2 = ChainQueryFilter::new().sequence_range(0..2);
        let query_3 = ChainQueryFilter::new().sequence_range(1..3);
        let query_4 = ChainQueryFilter::new().sequence_range(2..1000);

        assert_eq!(
            map_query(&query_1, &headers),
            [true, false, false, false, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_2, &headers),
            [true, true, false, false, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_3, &headers),
            [false, true, true, false, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_4, &headers),
            [false, false, true, true, true, true].to_vec()
        );
    }

    #[test]
    fn filter_by_multi() {
        let (headers, (entry_type_1, entry_type_2)) = fixtures();

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .header_type(HeaderType::EntryCreate)
                    .entry_type(entry_type_1.clone())
                    .sequence_range(0..1),
                &headers
            ),
            [true, false, false, false, false, false].to_vec()
        );
    }
}
