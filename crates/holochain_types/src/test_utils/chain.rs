//! Implements TestChainItem, a type used with isotest

use ::fixt::prelude::*;
use holo_hash::*;
use holochain_zome_types::prelude::*;
use std::ops::Range;

use crate::prelude::ChainItem;

/// The hash type for a [`TestChainItem`]
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::From,
    derive_more::Deref,
    derive_more::Into,
)]
pub struct TestChainHash(pub u32);

impl From<u8> for TestChainHash {
    fn from(u: u8) -> Self {
        Self(u as u32)
    }
}

impl From<i32> for TestChainHash {
    fn from(u: i32) -> Self {
        Self(u as u32)
    }
}

impl TestChainHash {
    fn forked(n: u8, i: u8) -> TestChainHash {
        TestChainHash(u32::from_le_bytes([n, i, 0, 0]))
    }
}

isotest::iso! {
    TestChainHash => |h| hash_from_u32(*h),
    ActionHash => |h| Self(u32::from_le_bytes(h.get_raw_32()[0..4].try_into().unwrap())),
    test_cases: [
        TestChainHash(0),
        TestChainHash(256),
        TestChainHash(u32::MAX)
    ],
    real_cases: [
        ActionHash::from_raw_32(vec![0; 32]),
        ActionHash::from_raw_32(vec![255; 32])
    ],
}

/// A test implementation of a minimal ChainItem which uses simple numbers for hashes
/// and always points back to the previous number
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TestChainItem {
    /// The sequence number
    pub seq: u32,
    /// The hash
    pub hash: TestChainHash,
    /// The timestamp
    pub timestamp: Timestamp,
    /// The previous hash, unless this is the first item
    pub prev: Option<TestChainHash>,
}

impl TestChainItem {
    /// Constructor for happy-path chains with no forking
    pub fn new(seq: u32) -> Self {
        Self {
            seq,
            hash: TestChainHash(seq),
            timestamp: Timestamp::from_micros((seq * 1000).into()),
            prev: seq.checked_sub(1).map(TestChainHash),
        }
    }

    /// Constructor for happy-path chains with no forking
    pub fn with_ts(seq: u32, us: i64) -> Self {
        Self {
            seq,
            hash: TestChainHash(seq),
            timestamp: Timestamp::from_micros(us),
            prev: seq.checked_sub(1).map(TestChainHash),
        }
    }

    /// Constructor chains with forks, with `i` representing a fork index
    pub fn forked(seq: u8, new_fork: u8, prev_fork: u8) -> Self {
        Self {
            seq: seq.into(),
            hash: TestChainHash::forked(seq, new_fork),
            timestamp: Timestamp(seq.into()),
            prev: seq
                .checked_sub(1)
                .map(|s| TestChainHash::forked(s, prev_fork)),
        }
    }
}

impl ChainItem for TestChainItem {
    type Hash = TestChainHash;

    fn seq(&self) -> u32 {
        self.seq
    }

    fn get_hash(&self) -> &Self::Hash {
        &self.hash
    }

    fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }

    fn prev_hash(&self) -> Option<&Self::Hash> {
        self.prev.as_ref()
    }

    fn to_display(&self) -> String {
        String::from("test chain item")
    }
}

impl AsRef<Self> for TestChainItem {
    fn as_ref(&self) -> &Self {
        self
    }
}

/// Create a hash from a slice by repeating the slice to fill out the array.
fn hash(i: &[u8]) -> Vec<u8> {
    let mut i = i.iter().copied().take(36).collect::<Vec<_>>();
    let num_needed = 36 - i.len();
    i.extend(std::iter::repeat_n(0, num_needed));
    i
}

/// Canonical way to construct a hash from a u32.
/// This is used in various places in our test code, and each must match.
pub fn hash_from_u32(i: u32) -> ActionHash {
    if i > u8::MAX as u32 {
        action_hash(&i.to_le_bytes())
    } else {
        action_hash(&[i as u8])
    }
}

/// Create a hash from a slice by repeating the slice to fill out the array
pub fn action_hash(i: &[u8]) -> ActionHash {
    ActionHash::from_raw_36(hash(i))
}

/// Create a hash from a slice by repeating the slice to fill out the array
pub fn agent_hash(i: &[u8]) -> AgentPubKey {
    AgentPubKey::from_raw_36(hash(i))
}

/// Create a hash from a slice by repeating the slice to fill out the array
pub fn entry_hash(i: &[u8]) -> EntryHash {
    EntryHash::from_raw_36(hash(i))
}

/// Create a chain per agent
pub fn agent_chain(ranges: &[(u8, Range<u32>)]) -> Vec<(AgentPubKey, Vec<TestChainItem>)> {
    ranges
        .iter()
        .map(|(a, range)| (agent_hash(&[*a]), chain(range.clone())))
        .collect()
}

/// Create a doubled chain per agent
pub fn agent_chain_doubled(ranges: &[(u8, Range<u32>)]) -> Vec<(AgentPubKey, Vec<TestChainItem>)> {
    ranges
        .iter()
        .map(|(a, range)| (agent_hash(&[*a]), doubled_chain(range.clone())))
        .collect()
}

/// Create a chain from a range where the first chain items
/// previous hash == that items hash.
pub fn chain(range: Range<u32>) -> Vec<TestChainItem> {
    range.map(TestChainItem::new).rev().collect()
}

/// Same as chain() but timestamp only increments every two items
pub fn doubled_chain(range: Range<u32>) -> Vec<TestChainItem> {
    range
        .map(|i| TestChainItem::with_ts(i, i as i64 / 2 * 1000))
        .rev()
        .collect()
}

/// Create a set of chains with forks where the first range
/// is the chain that all following ranges fork from.
// This is limited to u8s, because we need to ensure that there is enough room
// to make hashes that don't collide within the forks.
pub fn forked_chain(ranges: &[Range<u8>]) -> Vec<TestChainItem> {
    let mut out = Vec::new();
    for (i, range) in ranges.iter().enumerate() {
        let r = range
            .clone()
            .enumerate()
            .map(|(j, n)| {
                if j == 0 || i == 0 {
                    let prev = n.checked_sub(1).map(Into::into);
                    TestChainItem {
                        seq: n as u32,
                        hash: TestChainHash::forked(n, i as u8),
                        timestamp: Timestamp(n as i64 * 1000),
                        prev,
                    }
                } else {
                    let prev = n
                        .checked_sub(1)
                        .map(|n_sub_1| TestChainHash::forked(n_sub_1, i as u8));
                    TestChainItem {
                        seq: n as u32,
                        hash: TestChainHash::forked(n, i as u8),
                        timestamp: Timestamp(n as i64 * 1000),
                        prev,
                    }
                }
            })
            .rev();
        out.extend(r);
    }
    out.sort_unstable_by_key(|s| s.seq);
    out.reverse();
    out
}

/// Build a chain with gaps in it. Each range will make a chain even if there
/// are gaps.
pub fn gap_chain(ranges: &[Range<u32>]) -> Vec<TestChainItem> {
    let min = ranges.iter().map(|r| r.start).min().unwrap();
    let max = ranges.iter().map(|r| r.end).max().unwrap();
    chain(min..max)
        .into_iter()
        .filter(|i| ranges.iter().any(|r| r.contains(&i.seq)))
        .collect()
}

/// Produce an arbitrary SignedActionHashed from any ChainItem.
///
/// The SignedActionHashed will not be valid in any sense other than the
/// fields relevant to ChainItem.
pub fn chain_item_to_action(i: &impl ChainItem) -> SignedActionHashed {
    let action_seq = i.seq();
    let prev_action = i.prev_hash().cloned().map(Into::into);
    let hash: ActionHash = i.get_hash().clone().into();
    let mut action = fixt!(SignedActionHashed);
    match (action_seq, prev_action) {
        (_, None) => {
            let dna = fixt!(Dna);
            action.hashed.content = Action::Dna(dna);
            action.hashed.hash = hash;
        }
        (action_seq, Some(prev_action)) => {
            let mut create = fixt!(Create);
            create.action_seq = action_seq;
            create.prev_action = prev_action;
            create.timestamp = i.get_timestamp();
            action.hashed.content = Action::Create(create);
            action.hashed.hash = hash;
        }
    }
    action
}

/// Produce a sequence of AgentActivity ops from a Vec of ChainItems
pub fn chain_to_ops(chain: Vec<impl ChainItem>) -> Vec<RegisterAgentActivity> {
    chain
        .into_iter()
        .map(|i| {
            let mut op = RegisterAgentActivity {
                action: fixt!(SignedActionHashed),
                cached_entry: None,
            };
            op.action = chain_item_to_action(&i);
            op
        })
        .collect()
}
