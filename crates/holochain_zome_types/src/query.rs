//! Types for source chain queries

use crate::header::EntryType;
use crate::header::Header;
use crate::header::HeaderType;
use crate::warrant::Warrant;
use holo_hash::HeaderHash;
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
    // NB: if this filter is set, you can't verify the results, so don't
    //     use this in validation
    pub entry_type: Option<EntryType>,
    /// Filter by HeaderType
    // NB: if this filter is set, you can't verify the results, so don't
    //     use this in validation
    pub header_type: Option<HeaderType>,
    /// Include the entries in the elements
    pub include_entries: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// An agents chain elements returned from a agent_activity_query
pub struct AgentActivity {
    /// Valid headers on this chain.
    pub valid_activity: Vec<(u32, HeaderHash)>,
    /// Rejected headers on this chain.
    pub rejected_activity: Vec<(u32, HeaderHash)>,
    /// The status of this chain.
    pub status: ChainStatus,
    /// The highest chain header that has
    /// been observed by this authority.
    pub highest_observed: Option<HighestObserved>,
    /// Warrants about this AgentActivity.
    /// Placeholder for future.
    pub warrants: Vec<Warrant>,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// Get either the full activity or just the status of the chain
pub enum ActivityRequest {
    /// Just request the status of the chain
    Status,
    /// Request all the activity
    Full,
}

#[derive(Clone, Debug, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
/// The highest header sequence observed by this authority.
/// This also includes the headers at this sequence.
/// If there is more then one then there is a fork.
///
/// This type is to prevent headers being hidden by
/// withholding the previous header.
///
/// The information is tracked at the edge of holochain before
/// validation (but after drop checks).
pub struct HighestObserved {
    /// The highest sequence number observed.
    pub header_seq: u32,
    /// Hashes of any headers claiming to be at this
    /// header sequence.
    pub hash: Vec<HeaderHash>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// Status of the agent activity chain
// TODO: In the future we will most likely be replaced
// by warrants instead of Forked / Invalid so we can provide
// evidence of why the chain has a status.
pub enum ChainStatus {
    /// This authority has no information on the chain.
    Empty,
    /// The chain is valid as at this header sequence and header hash.
    Valid(ChainHead),
    /// Chain is forked.
    Forked(ChainFork),
    /// Chain is invalid because of this header.
    Invalid(ChainHead),
}

impl Default for ChainStatus {
    fn default() -> Self {
        ChainStatus::Empty
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// The header at the head of the complete chain.
/// This is as far as this authority can see a
/// chain with no gaps.
pub struct ChainHead {
    /// Sequence number of this chain head.
    pub header_seq: u32,
    /// Hash of this chain head
    pub hash: HeaderHash,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// The chain has been forked by these two headers
pub struct ChainFork {
    /// The point where the chain has forked.
    pub fork_seq: u32,
    /// The first header at this sequence position.
    pub first_header: HeaderHash,
    /// The second header at this sequence position.
    pub second_header: HeaderHash,
}

impl ChainQueryFilter {
    /// Create a no-op ChainQueryFilter which returns everything
    pub fn new() -> Self {
        Self {
            include_entries: false,
            ..Self::default()
        }
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

    /// Include the entries in the ElementsVec that is returned
    pub fn include_entries(mut self, include_entries: bool) -> Self {
        self.include_entries = include_entries;
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
                    .unwrap_or(false)
            })
            .unwrap_or(true);
        check_range && check_header_type && check_entry_type
    }
}

#[cfg(test)]
#[cfg(feature = "fixturators")]
mod tests {
    use crate::fixt::AppEntryTypeFixturator;
    use crate::fixt::*;
    use crate::header::EntryType;
    use crate::Header;
    use ::fixt::prelude::*;

    use super::ChainQueryFilter;

    /// Create three Headers with various properties.
    /// Also return the EntryTypes used to construct the first two headers.
    fn fixtures() -> [Header; 6] {
        let entry_type_1 = EntryType::App(fixt!(AppEntryType));
        let entry_type_2 = EntryType::AgentPubKey;

        let mut h1 = fixt!(Create);
        h1.entry_type = entry_type_1.clone();
        h1.header_seq = 0;

        let mut h2 = fixt!(Update);
        h2.entry_type = entry_type_2.clone();
        h2.header_seq = 1;

        let mut h3 = fixt!(CreateLink);
        h3.header_seq = 2;

        let mut h4 = fixt!(Create);
        h4.entry_type = entry_type_2.clone();
        h4.header_seq = 3;

        let mut h5 = fixt!(Update);
        h5.entry_type = entry_type_1.clone();
        h5.header_seq = 4;

        let mut h6 = fixt!(CreateLink);
        h6.header_seq = 5;

        let headers = [
            h1.into(),
            h2.into(),
            h3.into(),
            h4.into(),
            h5.into(),
            h6.into(),
        ];
        headers
    }

    fn map_query(query: &ChainQueryFilter, headers: &[Header]) -> Vec<bool> {
        headers.iter().map(|h| query.check(h)).collect::<Vec<_>>()
    }

    #[test]
    fn filter_by_entry_type() {
        let headers = fixtures();

        let query_1 =
            ChainQueryFilter::new().entry_type(headers[0].entry_type().unwrap().to_owned());
        let query_2 =
            ChainQueryFilter::new().entry_type(headers[1].entry_type().unwrap().to_owned());

        assert_eq!(
            map_query(&query_1, &headers),
            [true, false, false, false, true, false].to_vec()
        );
        assert_eq!(
            map_query(&query_2, &headers),
            [false, true, false, true, false, false].to_vec()
        );
    }

    #[test]
    fn filter_by_header_type() {
        let headers = fixtures();

        let query_1 = ChainQueryFilter::new().header_type(headers[0].header_type());
        let query_2 = ChainQueryFilter::new().header_type(headers[1].header_type());
        let query_3 = ChainQueryFilter::new().header_type(headers[2].header_type());

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
        let headers = fixtures();

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
        let headers = fixtures();

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .header_type(headers[0].header_type())
                    .entry_type(headers[0].entry_type().unwrap().clone())
                    .sequence_range(0..1),
                &headers
            ),
            [true, false, false, false, false, false].to_vec()
        );

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .header_type(headers[1].header_type())
                    .entry_type(headers[0].entry_type().unwrap().clone())
                    .sequence_range(0..1000),
                &headers
            ),
            [false, false, false, false, true, false].to_vec()
        );

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .entry_type(headers[0].entry_type().unwrap().clone())
                    .sequence_range(0..1000),
                &headers
            ),
            [true, false, false, false, true, false].to_vec()
        );
    }
}
