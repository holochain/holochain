use super::*;
use holo_hash::ActionHash;

fn hash(i: u8) -> ActionHash {
    ActionHash::from_raw_36(vec![i; 36])
}

#[test]
fn can_serialize() {
    let filter = ChainFilter::new(hash(0));
    let sb = SerializedBytes::try_from(&filter).unwrap();
    let result = ChainFilter::try_from(sb).unwrap();
    assert_eq!(filter, result);
}

#[test]
fn take_constructor_sets_take_limit() {
    let filter = ChainFilter::take(hash(0), 3);
    assert_eq!(filter.get_take(), Some(3));
}

#[test]
fn until_hash_constructor_sets_hash_limit() {
    let hash_limit = hash(1);
    let filter = ChainFilter::until_hash(hash(0), hash_limit.clone());
    assert_eq!(filter.get_until_hash(), Some(&hash_limit));
}
