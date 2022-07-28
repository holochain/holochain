use std::ops::Range;
use test_case::test_case;

use crate::{prelude::TestChainItem, test_utils::chain::*};

use super::*;

/// Create a hash from a u8.
fn hash(i: u8) -> TestHash {
    vec![i]
}

pub type TestHash = <TestChainItem as ChainItem>::Hash;
pub type TestFilter = ChainFilter<TestHash>;

/// Build a chain of RegisterAgentActivity and then run them through the
/// chain filter.
fn build_chain(c: Vec<TestChainItem>, filter: TestFilter) -> Vec<TestChainItem> {
    ChainFilterIter::new(filter, c).into_iter().collect()
}

#[test_case(1, 0, 0 => chain(0..0))]
#[test_case(1, 0, 1 => chain(0..1))]
#[test_case(1, 0, 10 => chain(0..1))]
#[test_case(2, 0, 10 => chain(0..1))]
#[test_case(2, 1, 10 => chain(0..2))]
#[test_case(10, 9, 10 => chain(0..10))]
/// Check taking n items works.
fn can_take_n(len: u8, position: u8, take: u32) -> Vec<TestChainItem> {
    let filter = TestFilter::new(hash(position)).take(take);
    build_chain(chain(0..len), filter)
}

#[test_case(1, 0, hash(0) => chain(0..1))]
#[test_case(1, 0, hash(1) => chain(0..1))]
#[test_case(2, 1, hash(1) => chain(1..2))]
#[test_case(10, 5, hash(1) => chain(1..6))]
#[test_case(10, 9, hash(0) => chain(0..10))]
/// Check taking until some hash works.
fn can_until_hash(len: u8, position: u8, until: TestHash) -> Vec<TestChainItem> {
    let filter = TestFilter::new(hash(position)).until(until);
    build_chain(chain(0..len), filter)
}

#[test_case(10, TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(10, TestFilter::new(hash(9)).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, TestFilter::new(hash(9)).take(20).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, TestFilter::new(hash(9)).take(20).take(2).until(hash(4)).until(hash(9)) => chain(9..10))]
/// Check take and until can be combined and the first to be
/// reached ends the iterator.
fn can_combine(len: u8, filter: TestFilter) -> Vec<TestChainItem> {
    build_chain(chain(0..len), filter)
}

#[test_case(&[0..10], TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..10, 7..10], TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..10, 7..10, 5..8, 3..7], TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
/// Check that forked chains are ignored.
fn can_ignore_forks(ranges: &[Range<u8>], filter: TestFilter) -> Vec<TestChainItem> {
    build_chain(forked_chain(ranges), filter)
}

#[test_case(&[0..10], TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(&[0..5, 6..10], TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(6..10))]
#[test_case(&[0..5, 6..7, 8..10], TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(8..10))]
#[test_case(&[0..5, 6..7, 8..10], TestFilter::new(hash(9)).take(3).until(hash(4)) => chain(8..10))]
/// Check the iterator will stop at a gap in the chain.
fn stop_at_gap(ranges: &[Range<u8>], filter: TestFilter) -> Vec<TestChainItem> {
    build_chain(gap_chain(ranges), filter)
}
