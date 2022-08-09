use std::ops::Range;

use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use holo_hash::ActionHash;
use holochain_zome_types::*;

use super::TestChainHash;
use super::TestChainItem;

fn forked_hash(n: u8, i: u8) -> TestChainHash {
    TestChainHash(n as u32 + (i as u32) * 256)
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

pub fn chain_item_to_action(u: &mut Unstructured, i: &impl ChainItem) -> SignedActionHashed {
    let action_seq = i.seq();
    let prev_action = i.prev_hash().cloned().map(Into::into);
    let hash: ActionHash = i.get_hash().clone().into();
    let mut action = SignedActionHashed::arbitrary(u).unwrap();
    match (action_seq, prev_action) {
        (0, _) => {
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
        _ => unreachable!(),
    }
    action
}

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
