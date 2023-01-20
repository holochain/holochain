use holo_hash::*;
use std::collections::HashMap;
use std::ops::Range;
use test_case::test_case;

use crate::test_utils::chain::*;

use super::*;

type TestHash = <TestChainItem as ChainItem>::Hash;
type TestFilter = ChainFilter<TestHash>;

/// Create a hash from a u32.
fn hash(i: u32) -> TestHash {
    i.into()
}

/// Build a chain of RegisterAgentActivity and then run them through the
/// chain filter.
fn build_chain(c: Vec<TestChainItem>, filter: TestFilter) -> Vec<TestChainItem> {
    ChainFilterIter::new(filter, c).into_iter().collect()
}

/// Useful for displaying diff of test_case failure.
/// See <https://github.com/frondeus/test-case/wiki/Syntax#function-validator>
fn pretty(expected: Vec<TestChainItem>) -> impl Fn(Vec<TestChainItem>) {
    move |actual: Vec<TestChainItem>| pretty_assertions::assert_eq!(actual, expected)
}
#[test_case(1, 0, 0 => chain(0..0))]
#[test_case(1, 0, 1 => chain(0..1))]
#[test_case(1, 0, 10 => chain(0..1))]
#[test_case(2, 0, 10 => chain(0..1))]
#[test_case(2, 1, 10 => chain(0..2))]
#[test_case(10, 9, 10 => chain(0..10))]
/// Check taking n items works.
fn can_take_n(len: u32, chain_top: u32, take: u32) -> Vec<TestChainItem> {
    let filter = TestFilter::new(hash(chain_top)).take(take);
    build_chain(chain(0..len), filter)
}

#[test_case(1, 0, hash(0) => chain(0..1))]
#[test_case(1, 0, hash(1) => chain(0..1))]
#[test_case(2, 1, hash(1) => chain(1..2))]
#[test_case(10, 5, hash(1) => using pretty(chain(1..6)))]
#[test_case(10, 9, hash(0) => using pretty(chain(0..10)))]
/// Check taking until some hash works.
fn can_until_hash(len: u32, chain_top: u32, until: TestHash) -> Vec<TestChainItem> {
    let filter = TestFilter::new(hash(chain_top)).until(until);
    build_chain(chain(0..len), filter)
}

#[test_case(10, TestFilter::new(hash(9)).take(10).until(hash(4)) => chain(4..10))]
#[test_case(10, TestFilter::new(hash(9)).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, TestFilter::new(hash(9)).take(20).take(2).until(hash(4)) => chain(8..10))]
#[test_case(10, TestFilter::new(hash(9)).take(20).take(2).until(hash(4)).until(hash(9)) => chain(9..10))]
/// Check take and until can be combined and the first to be
/// reached ends the iterator.
fn can_combine(len: u32, filter: TestFilter) -> Vec<TestChainItem> {
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
fn stop_at_gap(ranges: &[Range<u32>], filter: TestFilter) -> Vec<TestChainItem> {
    build_chain(gap_chain(ranges), filter)
}

fn matches_chain(a: &Vec<RegisterAgentActivity>, seq: &[u32]) -> bool {
    a.len() == seq.len()
        && a.iter()
            .map(|op| op.action.action().action_seq())
            .zip(seq)
            .all(|(a, b)| a == *b)
}

#[test_case(
    chain(0..3), ChainFilter::new(action_hash(&[1])), hash_to_seq(&[1])
    => matches MustGetAgentActivityResponse::Activity(a) if matches_chain(&a, &[1, 0]) ; "chain_top 1 chain 0 to 3")]
#[test_case(
    chain(10..20), ChainFilter::new(action_hash(&[15])).until(action_hash(&[10])), hash_to_seq(&[10, 15])
    => matches MustGetAgentActivityResponse::Activity(a) if matches_chain(&a, &[15, 14, 13, 12, 11, 10]) ; "chain_top 15 until 10 chain 10 to 20")]
#[test_case(
    chain(10..16), ChainFilter::new(action_hash(&[15])).until(action_hash(&[10])).take(2), hash_to_seq(&[10, 15])
    => matches MustGetAgentActivityResponse::Activity(a) if matches_chain(&a, &[15, 14]) ; "chain_top 15 until 10 take 2 chain 10 to 15")]
#[test_case(
    chain(1..6), ChainFilter::new(action_hash(&[5])).until(action_hash(&[0])).take(6), hash_to_seq(&[0, 5])
    => matches MustGetAgentActivityResponse::IncompleteChain ; "chain_top 5 until 0 take 6 chain 1 to 5")]
#[test_case(
    chain(0..5), ChainFilter::new(action_hash(&[5])).until(action_hash(&[0])).take(6), hash_to_seq(&[0, 5])
    => matches MustGetAgentActivityResponse::IncompleteChain ; "chain_top 5 until 0 take 6 chain 0 to 4")]
#[test_case(
    gap_chain(&[0..4, 5..10]), ChainFilter::new(action_hash(&[7])).until(action_hash(&[0])).take(8), hash_to_seq(&[0, 7])
    => matches MustGetAgentActivityResponse::IncompleteChain ; "chain_top 7 until 0 take 8 chain 0 to 3 then 5 to 10")]
#[test_case(
    gap_chain(&[0..4, 5..10]), ChainFilter::new(action_hash(&[7])).until(action_hash(&[5])).take(8), hash_to_seq(&[5, 7])
    => matches MustGetAgentActivityResponse::Activity(a) if matches_chain(&a, &[7, 6, 5]) ; "chain_top 7 until 5 take 8 chain 0 to 3 then 5 to 10")]
#[test_case(
    gap_chain(&[0..4, 5..10]), ChainFilter::new(action_hash(&[7])).until(action_hash(&[0])).take(3), hash_to_seq(&[0, 7])
    => matches MustGetAgentActivityResponse::Activity(a) if matches_chain(&a, &[7, 6, 5]) ; "chain_top 7 until 0 take 3 chain 0 to 3 then 5 to 10")]
#[test_case(
    forked_chain(&[0..6, 3..8]), ChainFilter::new(action_hash(&[5])).until(action_hash(&[0])).take(8), hash_to_seq(&[0, 5])
    => matches MustGetAgentActivityResponse::Activity(a) if matches_chain(&a, &[5, 4, 3, 2, 1, 0]) ; "chain_top 5 until 0 take 8 chain 0 to 5 and 3 to 7")]
#[test_case(
    forked_chain(&[0..6, 3..8]), ChainFilter::new(action_hash(&[7, 1])).take(8), |_| Some(7)
    => matches MustGetAgentActivityResponse::Activity(a) if matches_chain(&a, &[7, 6, 5, 4, 3, 2, 1, 0]) ; "chain_top (7,1) take 8 chain 0 to 5 and 3 to 7")]
#[test_case(
    forked_chain(&[4..6, 3..8]), ChainFilter::new(action_hash(&[5, 0])).until(action_hash(&[4, 1])), |h| if *h == action_hash(&[5, 0]) { Some(5) } else { Some(4) }
    => matches MustGetAgentActivityResponse::IncompleteChain ; "chain_top (5,0) until (4,1) chain (0,0) to (5,0) and (3,1) to (7,1)")]
fn test_filter_then_check(
    chain: Vec<TestChainItem>,
    filter: ChainFilter,
    mut f: impl FnMut(&ActionHash) -> Option<u32>,
) -> MustGetAgentActivityResponse {
    let chain = chain_to_ops(chain);
    match Sequences::find_sequences::<_, ()>(filter, |a| Ok(f(a))) {
        Ok(Sequences::Found(s)) => s.filter_then_check(chain),
        _ => unreachable!(),
    }
}

#[test_case(
    ChainFilter::new(action_hash(&[1])), |_| Some(0)
    => matches Sequences::Found(s) if *s.range() == (0..=0) ; "Can find chain_top 0")]
#[test_case(
    ChainFilter::new(action_hash(&[1])), |_| Some(1)
    => matches Sequences::Found(s) if *s.range() == (0..=1) ; "Can find chain_top 1")]
#[test_case(
    ChainFilter::new(action_hash(&[1])), |_| None
    => matches Sequences::ChainTopNotFound(_); "chain_top missing")]
#[test_case(
    ChainFilter::new(action_hash(&[1])), |_| Some(u32::MAX)
    => matches Sequences::Found(s) if *s.range() == (0..=u32::MAX) ; "Can find chain_top max")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).take(0), |_| Some(0)
    => matches Sequences::EmptyRange; "chain_top 0 take 0")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).take(0), |_| Some(100)
    => matches Sequences::EmptyRange; "chain_top 100 take 0")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).take(1), |_| Some(0)
    => matches Sequences::Found(s) if *s.range() == (0..=0) ; "chain_top 0 take 1")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).take(1), |_| Some(1)
    => matches Sequences::Found(s) if *s.range() == (1..=1) ; "chain_top 1 take 1")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).take(u32::MAX), |_| Some(u32::MAX)
    => matches Sequences::Found(s) if *s.range() == (1..=u32::MAX) ; "chain_top max take max")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).take(5), |_| Some(10)
    => matches Sequences::Found(s) if *s.range() == (6..=10) ; "chain_top 10 take 5")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).take(u32::MAX), |_| Some(1)
    => matches Sequences::Found(s) if *s.range() == (0..=1) ; "chain_top 1 take max")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).until(action_hash(&[1])), hash_to_seq(&[1])
    => matches Sequences::Found(s) if *s.range() == (1..=1) ; "chain_top 1 until 1")]
#[test_case(
    ChainFilter::new(action_hash(&[1])).until(action_hash(&[0])), hash_to_seq(&[0, 1])
    => matches Sequences::Found(s) if *s.range() == (0..=1) ; "chain_top 1 until 0")]
#[test_case(
    ChainFilter::new(action_hash(&u32::MAX.to_le_bytes())).until(action_hash(&[0])), hash_to_seq(&[0, u32::MAX])
    => matches Sequences::Found(s) if *s.range() == (0..=u32::MAX) ; "chain_top max until 0")]
#[test_case(
    ChainFilter::new(action_hash(&u32::MAX.to_le_bytes())).until(action_hash(&u32::MAX.to_le_bytes())), hash_to_seq(&[u32::MAX])
    => matches Sequences::Found(s) if *s.range() == (u32::MAX..=u32::MAX) ; "chain_top max until max")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).until(action_hash(&[5])), hash_to_seq(&[5, 10])
    => matches Sequences::Found(s) if *s.range() == (5..=10) ; "chain_top 10 until 5")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).until(action_hash(&[5])).until(action_hash(&[8])), hash_to_seq(&[5, 8, 10])
    => matches Sequences::Found(s) if *s.range() == (8..=10) ; "chain_top 10 until 5 until 8")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).until(action_hash(&[8])).until(action_hash(&[5])), hash_to_seq(&[5, 8, 10])
    => matches Sequences::Found(s) if *s.range() == (8..=10) ; "chain_top 10 until 8 until 5")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).until(action_hash(&[5])), hash_to_seq(&[10])
    => matches Sequences::Found(s) if *s.range() == (0..=10); "missing until")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).until(action_hash(&[5])).until(action_hash(&[8])), hash_to_seq(&[5, 10])
    => matches Sequences::Found(s) if *s.range() == (5..=10); "missing and present until")]
#[test_case(
    ChainFilter::new(action_hash(&[5])).until(action_hash(&[10])), hash_to_seq(&[5, 10])
    => matches Sequences::Found(s) if *s.range() == (0..=5); "chain top less than until")]
#[test_case(
    ChainFilter::new(action_hash(&[6])).until(action_hash(&[5])).until(action_hash(&[8])), hash_to_seq(&[5, 8, 6])
    => matches Sequences::Found(s) if *s.range() == (5..=6); "chain top greater than and less than until")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).until(action_hash(&[8])).take(5), hash_to_seq(&[8, 10])
    => matches Sequences::Found(s) if *s.range() == (8..=10) ; "chain_top 10 until 8 take 5")]
#[test_case(
    ChainFilter::new(action_hash(&[10])).until(action_hash(&[2])).take(5), hash_to_seq(&[2, 10])
    => matches Sequences::Found(s) if *s.range() == (6..=10) ; "chain_top 10 until 2 take 5")]
fn test_find_sequences(
    filter: ChainFilter,
    mut f: impl FnMut(&ActionHash) -> Option<u32>,
) -> Sequences {
    match Sequences::find_sequences::<_, ()>(filter, |a| Ok(f(a))) {
        Ok(r) => r,
        Err(_) => unreachable!(),
    }
}

fn hash_to_seq(hashes: &[u32]) -> impl FnMut(&ActionHash) -> Option<u32> {
    let map = hashes
        .iter()
        .map(|i| {
            let hash = hash_from_u32(*i);
            (hash, *i)
        })
        .collect::<HashMap<_, _>>();
    move |hash| map.get(hash).copied()
}
