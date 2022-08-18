use std::ops::Range;

use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use holo_hash::*;
use holochain_zome_types::*;

use super::TestChainHash;
use super::TestChainItem;

fn forked_hash(n: u8, i: u8) -> TestChainHash {
    TestChainHash(n as u32 + (i as u32) * 256)
}

/// Create a hash from a slice by repeating the slice to fill out the array.
fn hash(i: &[u8]) -> Vec<u8> {
    let mut i = i.iter().copied().take(36).collect::<Vec<_>>();
    let num_needed = 36 - i.len();
    i.extend(std::iter::repeat(0).take(num_needed));
    i
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

pub fn chain_to_ops(chain: Vec<impl ChainItem>) -> Vec<RegisterAgentActivity> {
    let mut u = Unstructured::new(&holochain_zome_types::NOISE);
    chain
        .into_iter()
        .map(|i| {
            let action_seq = i.seq();
            let prev_action = i.prev_hash().cloned().map(Into::into);
            let hash: ActionHash = i.get_hash().clone().into();
            let mut op = RegisterAgentActivity::arbitrary(&mut u).unwrap();
            match (action_seq, prev_action) {
                (0, _) => {
                    let dna = Dna::arbitrary(&mut u).unwrap();
                    op.action.hashed.content = Action::Dna(dna);
                    op.action.hashed.hash = hash;
                }
                (action_seq, Some(prev_action)) => {
                    let mut create = Create::arbitrary(&mut u).unwrap();
                    create.action_seq = action_seq;
                    create.prev_action = prev_action;
                    op.action.hashed.content = Action::Create(create);
                    op.action.hashed.hash = hash;
                }
                _ => unreachable!(),
            }
            op
        })
        .collect()
}
