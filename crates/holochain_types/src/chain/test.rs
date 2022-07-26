use holo_hash::ActionHash;
use std::ops::Range;
use test_case::test_case;

use crate::test_utils::chain::*;

use super::*;

/// Create a hash from a u8.
fn hash(i: u8) -> ActionHash {
    action_hash(&[i])
}

/// Build a chain of RegisterAgentActivity and then run them through the
/// chain filter.
fn build_chain(c: Vec<ChainItem>, filter: ChainFilter) -> Vec<ChainItem> {
    let data = chain_to_ops(c);
    ChainFilterIter::new(filter, data)
        .map(|op| ChainItem {
            action_seq: op.action.hashed.action_seq(),
            hash: op.action.action_address().clone(),
            prev_action: op.action.hashed.prev_action().cloned(),
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
fn can_take_n(len: u8, position: u8, take: u32) -> Vec<ChainItem> {
    let filter = ChainFilter::new(hash(position)).take(take);
    build_chain(chain(0..len), filter)
}

#[test_case(1, 0, hash(0) => chain(0..1))]
#[test_case(1, 0, hash(1) => chain(0..1))]
#[test_case(2, 1, hash(1) => chain(1..2))]
#[test_case(10, 5, hash(1) => chain(1..6))]
#[test_case(10, 9, hash(0) => chain(0..10))]
/// Check taking until some hash works.
fn can_until_hash(len: u8, position: u8, until: ActionHash) -> Vec<ChainItem> {
    let filter = ChainFilter::new(hash(position)).until(until);
    build_chain(chain(0..len), filter)
}

#[test_case(10, ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(10, ChainFilter::new(hash(9)).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, ChainFilter::new(hash(9)).take(20).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, ChainFilter::new(hash(9)).take(20).take(2).until(hash(4)).until(hash(9)) => chain(9..10))]
/// Check take and until can be combined and the first to be
/// reached ends the iterator.
fn can_combine(len: u8, filter: ChainFilter) -> Vec<ChainItem> {
    build_chain(chain(0..len), filter)
}

#[test_case(&[0..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..10, 7..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..10, 7..10, 5..8, 3..7], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
/// Check that forked chains are ignored.
fn can_ignore_forks(ranges: &[Range<u8>], filter: ChainFilter) -> Vec<ChainItem> {
    build_chain(forked_chain(ranges), filter)
}

#[test_case(&[0..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..5, 6..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(6..10))]
#[test_case(&[0..5, 6..7, 8..10], ChainFilter::new(hash(9)).take(10).until(hash(4)) => chain(8..10))]
#[test_case(&[0..5, 6..7, 8..10], ChainFilter::new(hash(9)).take(3).until(hash(4)) => chain(8..10))]
/// Check the iterator will stop at a gap in the chain.
fn stop_at_gap(ranges: &[Range<u8>], filter: ChainFilter) -> Vec<ChainItem> {
    build_chain(gap_chain(ranges), filter)
}
