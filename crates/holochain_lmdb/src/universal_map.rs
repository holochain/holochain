//! A UniversalMap is a strongly typed key-value store where the type of the key
//! also contains the type of the value which corresponds to that key. This allows
//! a KV store of hetereogeneous value types, with typed keys that can only access
//! a single value. Holochain uses it to store database references of different types.
//!
//! This could be a small standalone crate with a little more polish
//!
//! Thanks to Carmelo Piccione (@struktured) for the implementation which we ported here.

use shrinkwraprs::Shrinkwrap;
use std::any::Any;
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

/// Key type for Universal map.
#[derive(Clone, Debug)]
pub struct Key<K: Send + Sync, V>(K, PhantomData<V>);

/// The actual UniversalMap struct. See module-level documentation.
#[derive(Shrinkwrap, Debug)]
pub struct UniversalMap<K: Send + Sync>(HashMap<K, Box<dyn Any + Send + Sync>>);

impl<K: Send + Sync, V> Key<K, V> {
    /// Create a new Key.
    pub fn new(key: K) -> Self {
        Self(key, PhantomData)
    }

    /// Get a ref to the raw key.
    pub fn key(&self) -> &K {
        &self.0
    }
}

impl<K: Clone + Send + Sync, V> Key<K, V> {
    /// If the key is clone-able, swap it out for a specific value type.
    pub fn with_value_type<VV>(&self) -> Key<K, VV> {
        let key: Key<K, VV> = Key::new(self.0.clone());
        key
    }
}

impl<K: Hash + Eq + PartialEq + Send + Sync, V> From<K> for Key<K, V> {
    fn from(key: K) -> Self {
        Self::new(key)
    }
}

impl<K: Hash + Eq + Send + Sync> Default for UniversalMap<K> {
    fn default() -> Self {
        UniversalMap::new()
    }
}

impl<K: Eq + Hash + Send + Sync> UniversalMap<K> {
    /// Construct a new UniversalMap
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert a key/value pair into the UniversalMap.
    pub fn insert<V: 'static + Send + Sync>(
        &mut self,
        key: Key<K, V>,
        value: V,
    ) -> Option<Box<dyn Any + Send + Sync>> {
        self.0.insert(key.0, Box::new(value))
    }

    /// Get a value from the UniversalMap.
    pub fn get<V: 'static + Send + Sync>(&self, key: &Key<K, V>) -> Option<&V> {
        match self.0.get(&key.0) {
            Some(value) => value.downcast_ref::<V>(),
            None => None,
        }
    }

    /// Is this UniversalMap empty?
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the length of the UniversalMap.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn univ_map_can_get_entries() {
        let mut univ_map = UniversalMap::default();

        #[derive(Clone, Debug, PartialEq)]
        struct Val(String);

        let val = Val("ghi".to_string());
        let key: Key<_, u8> = Key::new("abc");
        let key2: Key<_, bool> = Key::new("def");
        let key3: Key<_, Val> = Key::new("ghi");
        univ_map.insert(key.clone(), 123);
        univ_map.insert(key2.clone(), true);
        univ_map.insert(key3.clone(), val.clone());
        assert_eq!(univ_map.get(&key), Some(&123));
        assert_eq!(univ_map.get(&key2), Some(&true));
        assert_eq!(univ_map.get(&key3), Some(&val));
    }
}
