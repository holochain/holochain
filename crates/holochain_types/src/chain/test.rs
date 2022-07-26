use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use holo_hash::ActionHash;
use holochain_zome_types::Action;
use holochain_zome_types::Create;
use std::ops::Range;
use test_case::test_case;

use holochain_zome_types::RegisterAgentActivity;

use super::*;

/// Create a hash from a u8.
fn hash(i: u8) -> ActionHash {
    ActionHash::from_raw_36(vec![i as u8; 36])
}

/// Create a different hash from two u8s.
fn diff_hash(i: u8, j: u8) -> ActionHash {
    let mut d = vec![i; 35];
    d.push(j);
    ActionHash::from_raw_36(d)
}

type PrevHash = ActionHash;

/// Create a chain from a range where the first chain items
/// previous hash == that items hash.
fn chain(range: Range<u8>) -> Vec<(u32, ActionHash, PrevHash)> {
    range
        .map(|i| {
            let prev = i
                .checked_sub(1)
                .map_or_else(|| hash(0), |i_sub_1| hash(i_sub_1));
            (i as u32, hash(i), prev)
        })
        .rev()
        .collect()
}

/// Create a set of chains with forks where the first range
/// is the chain that all following ranges fork from.
fn forked_chain(ranges: &[Range<u8>]) -> Vec<(u32, ActionHash, PrevHash)> {
    let mut out = Vec::new();
    for (i, range) in ranges.iter().enumerate() {
        let r = range
            .clone()
            .enumerate()
            .map(|(j, n)| {
                if j == 0 || i == 0 {
                    let prev = n
                        .checked_sub(1)
                        .map_or_else(|| hash(0), |n_sub_1| hash(n_sub_1));
                    (n as u32, hash(n), prev)
                } else {
                    let prev = n.checked_sub(1).map_or_else(
                        || diff_hash(0, i as u8),
                        |n_sub_1| diff_hash(n_sub_1, i as u8),
                    );
                    (n as u32, diff_hash(n, i as u8), prev)
                }
            })
            .rev();
        out.extend(r);
    }
    out.sort_unstable_by_key(|(s, _, _)| *s);
    out.reverse();
    eprintln!("{:?}", out);
    out
}

/// Build a chain with gaps in it. Each range will make a chain even if there
/// are gaps.
fn gap_chain(ranges: &[Range<u8>]) -> Vec<(u32, ActionHash, PrevHash)> {
    let min = ranges.iter().map(|r| r.start).min().unwrap();
    let max = ranges.iter().map(|r| r.end).max().unwrap();
    chain(min..max)
        .into_iter()
        .filter(|i| ranges.iter().any(|r| r.contains(&(i.0 as u8))))
        .collect()
}

/// Build a chain of RegisterAgentActivity and then run them through the
/// chain filter.
fn build_chain(
    c: Vec<(u32, ActionHash, PrevHash)>,
    filter: ChainFilter,
) -> Vec<(u32, ActionHash, PrevHash)> {
    let mut u = Unstructured::new(&holochain_zome_types::NOISE);
    let data = c.into_iter().map(|(seq, action_hash, prev_hash)| {
        let mut op = RegisterAgentActivity::arbitrary(&mut u).unwrap();
        let mut create = Create::arbitrary(&mut u).unwrap();
        create.action_seq = seq;
        create.prev_action = prev_hash;
        op.action.hashed.content = Action::Create(create);
        op.action.hashed.hash = action_hash;
        op
    });

    ChainFilterIter::new(filter, data)
        .map(|op| {
            (
                op.action.hashed.action_seq(),
                op.action.action_address().clone(),
                op.action
                    .hashed
                    .prev_action()
                    .filter(|_| op.action.hashed.action_seq() != 0)
                    .cloned()
                    .unwrap_or_else(|| hash(0)),
            )
        })
        .collect()
}

#[test_case(1, 0, 0 => chain(0..0))]
#[test_case(1, 0, 1 => chain(0..1))]
#[test_case(1, 0, 10 => chain(0..1))]
#[test_case(2, 0, 10 => chain(0..1))]
#[test_case(2, 1, 10 => chain(0..2))]
#[test_case(10, 9, 10 => chain(0..10))]
/// Check taking n items works.
fn can_take_n(len: u8, position: u8, take: u32) -> Vec<(u32, ActionHash, PrevHash)> {
    let filter = ChainFilter::new(hash(position)).take(take);
    build_chain(chain(0..len), filter)
}

#[test_case(1, 0, hash(0) => chain(0..1))]
#[test_case(1, 0, hash(1) => chain(0..1))]
#[test_case(2, 1, hash(1) => chain(1..2))]
#[test_case(10, 5, hash(1) => chain(1..6))]
#[test_case(10, 9, hash(0) => chain(0..10))]
/// Check taking until some hash works.
fn can_until_hash(len: u8, position: u8, until: ActionHash) -> Vec<(u32, ActionHash, PrevHash)> {
    let filter = ChainFilter::new(hash(position)).until(until);
    build_chain(chain(0..len), filter)
}

#[test_case(10, ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(10, ChainFilter::new(hash(9)).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, ChainFilter::new(hash(9)).take(20).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, ChainFilter::new(hash(9)).take(20).take(2).until(hash(4)).until(hash(9)) => chain(9..10))]
/// Check take and until can be combined and the first to be
/// reached ends the iterator.
fn can_combine(len: u8, filter: ChainFilter) -> Vec<(u32, ActionHash, PrevHash)> {
    build_chain(chain(0..len), filter)
}

#[test_case(&[0..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..10, 7..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..10, 7..10, 5..8, 3..7], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
/// Check that forked chains are ignored.
fn can_ignore_forks(ranges: &[Range<u8>], filter: ChainFilter) -> Vec<(u32, ActionHash, PrevHash)> {
    build_chain(forked_chain(ranges), filter)
}

#[test_case(&[0..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..5, 6..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(6..10))]
#[test_case(&[0..5, 6..7, 8..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(8..10))]
#[test_case(&[0..5, 6..7, 8..10], ChainFilter::new(hash(9)).take(3).until(hash(4)) => chain(8..10))]
/// Check the iterator will stop at a gap in the chain.
fn stop_at_gap(ranges: &[Range<u8>], filter: ChainFilter) -> Vec<(u32, ActionHash, PrevHash)> {
    build_chain(gap_chain(ranges), filter)
}
