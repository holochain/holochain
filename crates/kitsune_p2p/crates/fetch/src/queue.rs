use std::ops::Deref;

use indexmap::{map::Entry, IndexMap};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MapQueue<K: Eq + std::hash::Hash, V> {
    inner: IndexMap<K, V>,
    index: usize,
}

impl<K, V> Default for MapQueue<K, V>
where
    K: Eq + std::hash::Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Deref for MapQueue<K, V>
where
    K: Eq + std::hash::Hash,
{
    type Target = IndexMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<K, V> MapQueue<K, V>
where
    K: Eq + std::hash::Hash,
{
    pub(crate) fn new() -> Self {
        Self {
            inner: IndexMap::new(),
            index: 0,
        }
    }

    pub(crate) fn front(&mut self) -> Option<(&K, &mut V)> {
        if self.inner.is_empty() {
            return None;
        }

        let fetch_index = self.index;
        self.index = (self.index + 1) % self.inner.len();

        self.inner.get_index_mut(fetch_index)
    }

    pub(crate) fn entry(&mut self, key: K) -> Entry<K, V> {
        self.inner.entry(key)
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<V> {
        // Fast but does change order so the element that is currently last will take the position of this element
        self.inner.swap_remove(key)
    }

    pub(crate) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.inner.get_mut(key)
    }
}

impl<K, V> FromIterator<(K, V)> for MapQueue<K, V>
where
    K: Eq + std::hash::Hash,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self {
            inner: iter.into_iter().collect(),
            index: 0,
        }
    }
}
