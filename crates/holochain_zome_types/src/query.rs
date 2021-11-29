//! Types for source chain queries

use std::collections::HashMap;
use std::collections::HashSet;

use crate::header::EntryType;
use crate::header::HeaderType;
use crate::warrant::Warrant;
use crate::Element;
use crate::HeaderHashed;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
pub use holochain_serialized_bytes::prelude::*;

/// Defines several ways that queries can be restricted to a range.
/// Notably hash bounded ranges disambiguate forks whereas sequence indexes do
/// not as the same position can be found in many forks.
/// The reason that this does NOT use native rust range traits is that the hash
/// bounded queries MUST be inclusive otherwise the integrity and fork
/// disambiguation logic is impossible. An exclusive range bound that does not
/// include the final header tells us nothing about which fork to select
/// between N forks of equal length that proceed it. With an inclusive hash
/// bounded range the final header always points unambiguously at the "correct"
/// fork that the range is over. Start hashes are not needed to provide this
/// property so ranges can be hash terminted with a length of proceeding
/// elements to return only. Technically the seq bounded ranges do not imply
/// any fork disambiguation and so could be a range but for simplicity we left
/// the API symmetrical in boundedness across all enum variants.
/// @TODO It may be possible to provide/implement RangeBounds in the case that
/// a full sequence of elements/headers is provided but it would need to be
/// handled as inclusive first, to enforce the integrity of the query, then the
/// exclusiveness achieved by simply removing the final element after the fact.
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Clone, Debug)]
pub enum ChainQueryFilterSequenceRange {
    /// Do NOT apply any range filtering for this query.
    Unbounded,
    /// A range over source chain sequence numbers.
    /// This is ambiguous over forking histories and so should NOT be used in
    /// validation logic.
    /// Inclusive start, inclusive end.
    HeaderSeqRange(u32, u32),
    /// A range over source chain header hashes.
    /// This CAN be used in validation logic as forks are disambiguated.
    /// Inclusive start and end (unlike std::ops::Range).
    HeaderHashRange(HeaderHash, HeaderHash),
    /// The terminating header hash and N preceeding elements.
    /// N = 0 returns only the element with this `HeaderHash`.
    /// This CAN be used in validation logic as forks are not possible when
    /// "looking up" towards genesis from some `HeaderHash`.
    HeaderHashTerminated(HeaderHash, u32),
}

impl Default for ChainQueryFilterSequenceRange {
    fn default() -> Self {
        Self::Unbounded
    }
}

/// Query arguments
#[derive(
    serde::Serialize, serde::Deserialize, SerializedBytes, Default, PartialEq, Clone, Debug,
)]
#[non_exhaustive]
pub struct ChainQueryFilter {
    /// Limit the results to a range of elements according to their headers.
    pub sequence_range: ChainQueryFilterSequenceRange,
    /// Filter by EntryType
    // NB: if this filter is set, you can't verify the results, so don't
    //     use this in validation
    pub entry_type: Option<EntryType>,
    /// Filter by a list of `EntryHash`.
    pub entry_hashes: Option<HashSet<EntryHash>>,
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
    pub fn sequence_range(mut self, sequence_range: ChainQueryFilterSequenceRange) -> Self {
        self.sequence_range = sequence_range;
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

    /// If the sequence range supports fork disambiguation, apply it to remove
    /// headers that are not in the correct branch.
    pub fn headers_without_forks(&self, headers: Vec<HeaderHashed>) -> Vec<HeaderHashed> {
        match &self.sequence_range {
            ChainQueryFilterSequenceRange::Unbounded => headers,
            ChainQueryFilterSequenceRange::HeaderSeqRange(start, end) => headers
                .into_iter()
                .filter(|header| *start <= header.header_seq() && header.header_seq() <= *end)
                .collect(),
            ChainQueryFilterSequenceRange::HeaderHashRange(start, end) => {
                let mut header_hashmap = headers
                    .iter()
                    .map(|header| (header.as_hash().clone(), header))
                    .collect::<HashMap<HeaderHash, &HeaderHashed>>();
                let mut filtered_headers = Vec::new();
                let mut maybe_next_header = header_hashmap.remove(&end);
                while let Some(next_header) = maybe_next_header {
                    maybe_next_header = header_hashmap.remove(next_header.as_hash());
                    filtered_headers.push(next_header.clone());
                    // This comes after the push to make the range inclusive.
                    if next_header.as_hash() == start {
                        break;
                    }
                }
                filtered_headers
            }
            ChainQueryFilterSequenceRange::HeaderHashTerminated(end, n) => {
                let mut header_hashmap = headers
                    .iter()
                    .map(|header| (header.as_hash().clone(), header))
                    .collect::<HashMap<HeaderHash, &HeaderHashed>>();
                let mut filtered_headers = Vec::new();
                let mut maybe_next_header = header_hashmap.remove(&end);
                let mut i = 0;
                while let Some(next_header) = maybe_next_header {
                    maybe_next_header = header_hashmap.remove(next_header.as_hash());
                    filtered_headers.push(next_header.clone());
                    // This comes after the push to make the range inclusive.
                    if i == *n {
                        break;
                    }
                    i = i + 1;
                }
                filtered_headers
            }
        }
    }

    /// Filter a vector of hashed headers according to the query.
    pub fn filter_headers(&self, headers: Vec<HeaderHashed>) -> Vec<HeaderHashed> {
        self.headers_without_forks(headers)
            .into_iter()
            .filter(|header| {
                self.header_type
                    .as_ref()
                    .map(|header_type| header.header_type() == *header_type)
                    .unwrap_or(true)
                    && self
                        .entry_type
                        .as_ref()
                        .map(|entry_type| header.entry_type() == Some(&entry_type))
                        .unwrap_or(true)
                    && self
                        .entry_hashes
                        .as_ref()
                        .map(|entry_hashes| match header.entry_hash() {
                            Some(entry_hash) => entry_hashes.contains(entry_hash),
                            None => false,
                        })
                        .unwrap_or(true)
            })
            .collect()
    }

    /// Filter a vector of elements according to the query.
    pub fn filter_elements(&self, elements: Vec<Element>) -> Vec<Element> {
        let headers = self.filter_headers(
            elements
                .iter()
                .map(|element| element.header_hashed())
                .cloned()
                .collect(),
        );
        let header_hashset = headers
            .iter()
            .map(|header| header.as_hash().clone())
            .collect::<HashSet<HeaderHash>>();
        elements
            .into_iter()
            .filter(|element| header_hashset.contains(element.header_hashed().as_hash()))
            .collect()
    }
}

#[cfg(test)]
#[cfg(feature = "fixturators")]
mod tests {
    use crate::fixt::AppEntryTypeFixturator;
    use crate::fixt::*;
    use crate::header::EntryType;
    use crate::HeaderHashed;
    use ::fixt::prelude::*;
    use crate::ChainQueryFilterSequenceRange;
    use super::ChainQueryFilter;

    /// Create three Headers with various properties.
    /// Also return the EntryTypes used to construct the first two headers.
    fn fixtures() -> [HeaderHashed; 7] {
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

        // Cheeky forker!
        let mut h4a = fixt!(Create);
        h4a.entry_type = entry_type_1.clone();
        h4a.header_seq = 3;

        let mut h5 = fixt!(Update);
        h5.entry_type = entry_type_1.clone();
        h5.header_seq = 4;

        let mut h6 = fixt!(CreateLink);
        h6.header_seq = 5;

        let headers = [
            HeaderHashed::from_content_sync(h1.into()),
            HeaderHashed::from_content_sync(h2.into()),
            HeaderHashed::from_content_sync(h3.into()),
            HeaderHashed::from_content_sync(h4.into()),
            HeaderHashed::from_content_sync(h4a.into()),
            HeaderHashed::from_content_sync(h5.into()),
            HeaderHashed::from_content_sync(h6.into()),
        ];
        headers
    }

    fn map_query(query: &ChainQueryFilter, headers: &[HeaderHashed]) -> Vec<bool> {
        let filtered = query.filter_headers(headers.to_vec());
        headers
            .iter()
            .map(|h| filtered.contains(h))
            .collect::<Vec<_>>()
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

        dbg!(&headers);

        let query_1 = ChainQueryFilter::new()
            .sequence_range(ChainQueryFilterSequenceRange::HeaderSeqRange(0, 0));
        let query_2 = ChainQueryFilter::new()
            .sequence_range(ChainQueryFilterSequenceRange::HeaderSeqRange(0, 1));
        let query_3 = ChainQueryFilter::new()
            .sequence_range(ChainQueryFilterSequenceRange::HeaderSeqRange(1, 2));
        let query_4 = ChainQueryFilter::new()
            .sequence_range(ChainQueryFilterSequenceRange::HeaderSeqRange(2, 999));

        assert_eq!(
            map_query(&query_1, &headers),
            [true, false, false, false, false, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_2, &headers),
            [true, true, false, false, false, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_3, &headers),
            [false, true, true, false, false, false, false].to_vec()
        );
        assert_eq!(
            map_query(&query_4, &headers),
            [false, false, true, true, true, true, true].to_vec()
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
                    .sequence_range(ChainQueryFilterSequenceRange::HeaderSeqRange(0, 0)),
                &headers
            ),
            [true, false, false, false, false, false].to_vec()
        );

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .header_type(headers[1].header_type())
                    .entry_type(headers[0].entry_type().unwrap().clone())
                    .sequence_range(ChainQueryFilterSequenceRange::HeaderSeqRange(0, 999)),
                &headers
            ),
            [false, false, false, false, true, false].to_vec()
        );

        assert_eq!(
            map_query(
                &ChainQueryFilter::new()
                    .entry_type(headers[0].entry_type().unwrap().clone())
                    .sequence_range(ChainQueryFilterSequenceRange::HeaderSeqRange(0, 999)),
                &headers
            ),
            [true, false, false, false, true, false].to_vec()
        );
    }
}
