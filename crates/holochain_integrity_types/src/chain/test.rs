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
fn take_n_is_min() {
    assert_eq!(
        ChainFilter::new(hash(0)).take(0),
        ChainFilter::new(hash(0)).take(0).take(5)
    );
    assert_eq!(
        ChainFilter::new(hash(0)).take(0),
        ChainFilter::new(hash(0)).take(5).take(0)
    );
}

#[test]
fn until_hash_is_a_set() {
    assert_eq!(
        ChainFilter::new(hash(0)).until(hash(0)).until(hash(1)),
        ChainFilter::new(hash(0))
            .until(hash(0))
            .until(hash(0))
            .until(hash(1))
            .until(hash(1)),
    );
}
