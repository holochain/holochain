use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use holochain_zome_types::NOISE;
use test_case::test_case;

use super::*;

#[test_case(1)]
#[test_case(2)]
#[test_case(u32::MAX)]
#[test_case(u32::MAX - 1)]
fn prev_is_empty_new_is_zero_check_empty(n: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    new_bounds.integrated = Some(n);
    // () -> (n)
    assert!(!prev_is_empty_new_is_zero(None, &new_bounds));
    // (()) -> (n)
    assert!(!prev_is_empty_new_is_zero(Some(&prev_bounds), &new_bounds));
    prev_bounds.integrated = Some(5);
    // (a) -> (n)
    assert!(prev_is_empty_new_is_zero(Some(&prev_bounds), &new_bounds));
}
#[test]
fn prev_is_empty_new_is_zero_check_zero() {
    let prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    new_bounds.integrated = Some(0);
    // () -> (0)
    assert!(prev_is_empty_new_is_zero(None, &new_bounds));
    // (()) -> (0)
    assert!(prev_is_empty_new_is_zero(Some(&prev_bounds), &new_bounds));
}

#[test_case(0)]
#[test_case(1)]
#[test_case(u32::MAX - 1)]
fn integrated_is_consecutive_check_n(n: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    prev_bounds.integrated = Some(n);
    new_bounds.integrated = Some(n + 1);
    // (n) -> (n+1)
    assert!(integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
    prev_bounds.integrated = None;
    // () -> (n)
    assert!(integrated_is_consecutive(None, &new_bounds));
    // (()) -> (n)
    assert!(integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
}

#[test_case(0)]
#[test_case(1)]
#[test_case(u32::MAX)]
fn integrated_is_consecutive_check_empty(n: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let new_bounds = ActivityBounds::default();
    // (n) -> ()
    prev_bounds.integrated = Some(n);
    assert!(integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
}

#[test_case(0, 2)]
#[test_case(0, u32::MAX)]
#[test_case(1, 3)]
#[test_case(1, u32::MAX)]
#[test_case(u32::MAX - 2, u32::MAX)]
#[test_case(u32::MAX, 0)]
#[test_case(u32::MAX - 1, 0)]
#[test_case(1, 0)]
#[test_case(2, 0)]
#[test_case(2, 1)]
#[test_case(3, 1)]
fn integrated_is_consecutive_check_finds_gaps(s: u32, e: u32) {
    let mut prev_bounds = ActivityBounds::default();
    let mut new_bounds = ActivityBounds::default();
    // (s) -> (e)
    prev_bounds.integrated = Some(s);
    new_bounds.integrated = Some(e);
    assert!(!integrated_is_consecutive(Some(&prev_bounds), &new_bounds));
}

#[test]
fn can_accept_ready_in_random_order() {
    use rand::prelude::*;
    let mut activity = HashMap::new();
    let mut u = Unstructured::new(&NOISE);
    let mut rand = rand::thread_rng();
    let mut sequence: Vec<_> = (0..10).collect();
    sequence.shuffle(&mut rand);
    let author = AgentPubKey::arbitrary(&mut u).unwrap();

    let mut new_bounds = ActivityBounds::default();
    let mut spent = Vec::with_capacity(sequence.len());
    for n in sequence {
        spent.push(n);
        spent.sort_unstable();
        new_bounds.ready_to_integrate = Some(n);
        update_activity(&mut activity, &author, &new_bounds).unwrap();
        let current_top = spent
            .iter()
            .zip(spent.iter().skip(1))
            .find(|(a, b)| **b != **a + 1)
            .map(|(a, _)| *a)
            .unwrap_or_else(|| *spent.last().unwrap());
        match &spent[..] {
            [0] => {
                assert_eq!(
                    activity.get(&author).unwrap().ready_to_integrate.unwrap(),
                    0
                );
            }
            [x] if *x > 0 => {
                assert_eq!(
                    activity.get(&author).unwrap().out_of_order.first().unwrap(),
                    x
                );
            }
            _ => {
                if spent.iter().any(|i| *i == 0) {
                    assert_eq!(
                        activity.get(&author).unwrap().ready_to_integrate.unwrap(),
                        current_top
                    );
                } else {
                    assert_eq!(activity.get(&author).unwrap().out_of_order, spent);
                }
            }
        }
    }
}

type AS = ActivityState;

#[test_case(AS::new(), None => AS::new())]
#[test_case(AS::new(), Some(0) => AS::new().ready(0))]
#[test_case(AS::new(), Some(1) => AS::new().out(vec![1]))]
#[test_case(AS::new().ready(0), Some(1) => AS::new().ready(1))]
#[test_case(AS::new().ready(0), Some(2) => AS::new().ready(0).out(vec![2]))]
#[test_case(AS::new().ready(0).out(vec![2]), Some(1) => AS::new().ready(2))]
#[test_case(AS::new().ready(0).out(vec![2, 3, 4]), Some(1) => AS::new().ready(4))]
#[test_case(AS::new().ready(0).out(vec![3, 4, 5]), Some(1) => AS::new().ready(1).out(vec![3, 4, 5]))]
#[test_case(AS::new().ready(0).out(vec![3, 4, 5]), Some(2) => AS::new().ready(0).out(vec![2, 3, 4, 5]))]
#[test_case(AS::new().integrated(0), Some(1) => AS::new().integrated(0).ready(1))]
#[test_case(AS::new().integrated(0), Some(2) => AS::new().integrated(0).out(vec![2]))]
#[test_case(AS::new().integrated(0).out(vec![2]), Some(1) => AS::new().integrated(0).ready(2))]
#[test_case(AS::new().integrated(0).out(vec![2, 3, 4]), Some(1) => AS::new().integrated(0).ready(4))]
#[test_case(AS::new().integrated(0).out(vec![3, 4, 5]), Some(1) => AS::new().integrated(0).ready(1).out(vec![3, 4, 5]))]
#[test_case(AS::new().integrated(0).out(vec![3, 4, 5]), Some(2) => AS::new().integrated(0).out(vec![2, 3, 4, 5]))]
#[test_case(AS::new().out(vec![0]), None => AS::new().ready(0))]
#[test_case(AS::new().integrated(0).ready(0), None => AS::new().integrated(0))]
fn update_ready_to_integrate_test(
    mut state: ActivityState,
    new_ready: Option<u32>,
) -> ActivityState {
    update_ready_to_integrate(&mut state, new_ready);
    state
}

#[test_case(vec![] => (None, vec![]))]
#[test_case(vec![0] => (Some(0), vec![]))]
#[test_case(vec![0, 1] => (Some(1), vec![]))]
#[test_case(vec![0, 1, 2] => (Some(2), vec![]))]
#[test_case(vec![0, 1, 3] => (Some(1), vec![3]))]
#[test_case(vec![0, 1, 3, 4] => (Some(1), vec![3, 4]))]
#[test_case(vec![0, 3, 4] => (Some(0), vec![3, 4]))]
fn find_consecutive_test(mut out_of_order: Vec<u32>) -> (Option<u32>, Vec<u32>) {
    let r = find_consecutive(&mut out_of_order);
    (r, out_of_order)
}
