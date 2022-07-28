use std::ops::Range;

use super::TestChainItem;
use super::TestChainHash;


fn forked_hash(n: u8, i: u8) -> TestChainHash {
    if i == 0 {
        vec![n]
    } else {
        vec![n, i]
    }
}

/// Create a chain per agent
pub fn agent_chain(ranges: &[(u8, Range<u8>)]) -> Vec<(TestChainHash, Vec<TestChainItem>)> {
    ranges
        .iter()
        .map(|(a, range)| (vec![*a], chain(range.clone())))
        .collect()
}

/// Create a chain from a range where the first chain items
/// previous hash == that items hash.
pub fn chain(range: Range<u8>) -> Vec<TestChainItem> {
    range
        .map(|i| {
            TestChainItem::new(i)
        })
        .rev()
        .collect()
}

/// Create a set of chains with forks where the first range
/// is the chain that all following ranges fork from.
pub fn forked_chain(ranges: &[Range<u8>]) -> Vec<TestChainItem> {
    let mut out = Vec::new();
    for (i, range) in ranges.iter().enumerate() {
        let r = range
            .clone()
            .enumerate()
            .map(|(j, n)| {
                if j == 0 || i == 0 {
                    let prev = n.checked_sub(1).map(|n_sub_1| vec![n_sub_1]);
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
pub fn gap_chain(ranges: &[Range<u8>]) -> Vec<TestChainItem> {
    let min = ranges.iter().map(|r| r.start).min().unwrap();
    let max = ranges.iter().map(|r| r.end).max().unwrap();
    chain(min..max)
        .into_iter()
        .filter(|i| ranges.iter().any(|r| r.contains(&(i.seq as u8))))
        .collect()
}
