//! Implements TestChainItem, a type used with isotest

use std::ops::Range;

use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use holo_hash::*;
use holochain_zome_types::*;

use crate::prelude::ChainItem;

/// The hash type for a [`TestChainItem`]
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
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

isotest::iso! {
    TestChainHash => |h| {
        let bytes: Vec<u8> = h.0.to_le_bytes().iter().cycle().take(32).copied().collect();
        ActionHash::from_raw_32(bytes)
    },
    ActionHash => |h| Self(u32::from_le_bytes(h.get_raw_32()[0..4].try_into().unwrap()))
}

/// A test implementation of a minimal ChainItem which uses simple numbers for hashes
/// and always points back to the previous number
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TestChainItem {
    /// The sequence number
    pub seq: u32,
    /// The hash
    pub hash: TestChainHash,
    /// The previous hash, unless this is the first item
    pub prev: Option<TestChainHash>,
}

impl TestChainItem {
    /// Constructor for happy-path chains with no forking
    pub fn new(seq: u32) -> Self {
        Self {
            seq,
            hash: TestChainHash(seq),
            prev: seq.checked_sub(1).map(TestChainHash),
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

    fn prev_hash(&self) -> Option<&Self::Hash> {
        self.prev.as_ref()
    }
}

impl AsRef<Self> for TestChainItem {
    fn as_ref(&self) -> &Self {
        self
    }
}

fn forked_hash(n: u8, i: u8) -> TestChainHash {
    TestChainHash(n as u32 + (i as u32) * 256)
}

pub type TestHash = <TestChainItem as ChainItem>::Hash;
pub type TestFilter = ChainFilter<TestHash>;

/// Create a hash from a u32.
fn hash(i: u32) -> TestHash {
    i.into()
}

pub fn action_hash(i: &[u8]) -> ActionHash {
    ActionHash::from_raw_36(hash(i))
}

pub fn agent_hash(i: &[u8]) -> AgentPubKey {
    AgentPubKey::from_raw_36(hash(i))
}

pub fn entry_hash(i: &[u8]) -> EntryHash {
    EntryHash::from_raw_36(hash(i))
}

/// Create a chain per agent
pub fn agent_chain(ranges: &[(u8, Range<u8>)]) -> Vec<(AgentPubKey, Vec<ChainItem>)> {
    ranges
        .iter()
        .map(|(a, range)| (agent_hash(&[*a]), chain(range.clone())))
        .collect()
}

/// Create a chain from a range where the first chain items
/// previous hash == that items hash.
pub fn chain(range: Range<u32>) -> Vec<TestChainItem> {
    range.map(TestChainItem::new).rev().collect()
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
                        hash: forked_hash(n as u8, i as u8),
                        prev,
                    }
                } else {
                    let prev = n
                        .checked_sub(1)
                        .map(|n_sub_1| forked_hash(n_sub_1, i as u8));
                    TestChainItem {
                        seq: n as u32,
                        hash: forked_hash(n, i as u8),
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
pub fn chain_item_to_action(u: &mut Unstructured, i: &impl ChainItem) -> SignedActionHashed {
    let action_seq = i.seq();
    let prev_action = i.prev_hash().cloned().map(Into::into);
    let hash: ActionHash = i.get_hash().clone().into();
    let mut action = SignedActionHashed::arbitrary(u).unwrap();
    match (action_seq, prev_action) {
        (_, None) => {
            let dna = Dna::arbitrary(u).unwrap();
            action.hashed.content = Action::Dna(dna);
            action.hashed.hash = hash;
        }
        (action_seq, Some(prev_action)) => {
            let mut create = Create::arbitrary(u).unwrap();
            create.action_seq = action_seq;
            create.prev_action = prev_action;
            action.hashed.content = Action::Create(create);
            action.hashed.hash = hash;
        }
    }
    action
}

/// Produce a sequence of AgentActivity ops from a Vec of ChainItems
pub fn chain_to_ops(chain: Vec<impl ChainItem>) -> Vec<RegisterAgentActivity> {
    let mut u = Unstructured::new(&holochain_zome_types::NOISE);
    chain
        .into_iter()
        .map(|i| {
            let mut op = RegisterAgentActivity::arbitrary(&mut u).unwrap();
            op.action = chain_item_to_action(&mut u, &i);
            op
        })
        .collect()
}

isotest::iso! {
    TestChainItem => |i| {
        let mut u = Unstructured::new(&holochain_zome_types::NOISE);
        chain_item_to_action(&mut u, &i)
    },
    SignedActionHashed => |a| {
        TestChainItem {
            seq: a.seq(),
            hash: TestChainHash::test(a.get_hash()),
            prev: a.prev_hash().map(TestChainHash::test),
        }
    }
}

#[cfg(test)]
#[test_case::test_case(0)]
#[test_case::test_case(65536)]
fn test_hash_roundtrips(u: u32) {
    let h1 = TestChainHash(u);
    let h2 = ActionHash::from_raw_32(u.to_le_bytes().iter().cycle().take(32).copied().collect());
    isotest::test_iso_invariants(h1, h2);
}

#[cfg(test)]
#[test_case::test_case(0, 0, None)]
#[test_case::test_case(0, 0, Some(0))]
#[test_case::test_case(1, 1, Some(0))]
#[test_case::test_case(1, 1, None => ignore)] // this case is unrepresentable with these types
fn test_chain_item_roundtrips(seq: u32, hash: u32, prev: Option<u32>) {
    use ::fixt::prelude::*;
    let item = TestChainItem {
        seq,
        hash: hash.into(),
        prev: prev.map(Into::into),
    };
    let action = fixt!(SignedActionHashed);
    isotest::test_iso_invariants(item, action);
}
