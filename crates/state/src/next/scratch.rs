use std::collections::BTreeMap;

#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
#[shrinkwrap(mutable)]
pub struct Scratch<V>(pub BTreeMap<Vec<u8>, ScratchOp<V>>);

impl<V> Scratch<V> {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

/// Transactional operations on a KV store
/// Put: add or replace this KV
/// Delete: remove the KV
#[derive(Clone, Debug, PartialEq)]
pub enum ScratchOp<V> {
    Put(Box<V>),
    Delete,
}
