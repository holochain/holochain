use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use holo_hash::{AnyDhtHash, DhtOpHash, HasHash};
use holochain_types::dht_op::DhtOpHashed;

/// Dependencies required for app validating an op.
pub struct ValidationDependencies {
    /// Missing hashes that are being fetched, along with
    /// the last Instant a fetch was attempted
    pub(super) missing_hashes: HashMap<AnyDhtHash, Instant>,
    /// Dependencies that are missing to app validate an op.
    pub(super) hashes_missing_for_op: HashMap<DhtOpHash, HashSet<AnyDhtHash>>,
}

impl Default for ValidationDependencies {
    fn default() -> Self {
        ValidationDependencies::new()
    }
}

impl ValidationDependencies {
    const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

    pub fn new() -> Self {
        Self {
            missing_hashes: HashMap::new(),
            hashes_missing_for_op: HashMap::new(),
        }
    }

    pub fn insert_missing_hash(&mut self, hash: AnyDhtHash) -> Option<Instant> {
        self.missing_hashes.insert(hash, Instant::now())
    }

    pub fn remove_missing_hash(&mut self, hash: &AnyDhtHash) {
        self.missing_hashes.remove(hash);
    }

    // filter out hashes that are known to be missing
    pub fn get_new_hashes_to_fetch(&mut self, hashes: Vec<AnyDhtHash>) -> Vec<AnyDhtHash> {
        hashes
            .into_iter()
            .filter(|hash| {
                let hash_present = self.insert_missing_hash(hash.clone());
                hash_present.is_none()
            })
            .collect()
    }

    pub fn fetch_missing_hashes_timed_out(&self) -> bool {
        if self.missing_hashes.is_empty() {
            return false;
        }
        self.missing_hashes
            .iter()
            .all(|(_, instant)| instant.elapsed() > Self::FETCH_TIMEOUT)
    }

    pub fn insert_hash_missing_for_op(&mut self, dht_op_hash: DhtOpHash, hash: AnyDhtHash) {
        self.hashes_missing_for_op
            .entry(dht_op_hash)
            .and_modify(|hashes| {
                hashes.insert(hash.clone());
            })
            .or_insert_with(|| {
                let mut set = HashSet::new();
                set.insert(hash.clone());
                set
            });
    }

    pub fn remove_hash_missing_for_op(&mut self, dht_op_hash: DhtOpHash, hash: &AnyDhtHash) {
        self.hashes_missing_for_op
            .entry(dht_op_hash.clone())
            .and_modify(|hashes| {
                hashes.remove(hash);
            });

        // if there are no hashes left for this dht op hash,
        // remove the entry
        if let Some(hashes) = self.hashes_missing_for_op.get(&dht_op_hash) {
            if hashes.is_empty() {
                self.hashes_missing_for_op.remove(&dht_op_hash);
            }
        }
    }

    // filter out ops that have missing dependencies
    pub fn filter_ops_missing_dependencies(&self, ops: Vec<DhtOpHashed>) -> Vec<DhtOpHashed> {
        ops.into_iter()
            .filter(|op| !self.hashes_missing_for_op.contains_key(op.as_hash()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use fixt::fixt;
    use holo_hash::{fixt::AnyDhtHashFixturator, AnyDhtHash};

    use super::ValidationDependencies;

    #[test]
    fn get_new_hashes_to_fetch() {
        let mut validation_dependencies = ValidationDependencies::default();

        // no missing hashes present
        // hash 1 should be only new hash
        let hash_1 = fixt!(AnyDhtHash);
        let filtered_hashes_to_fetch =
            validation_dependencies.get_new_hashes_to_fetch(vec![hash_1.clone()]);
        assert_eq!(filtered_hashes_to_fetch, vec![hash_1.clone()]);

        // hash 1 is still present
        // new hashes should be empty
        let filtered_hashes_to_fetch =
            validation_dependencies.get_new_hashes_to_fetch(vec![hash_1.clone()]);
        assert_eq!(filtered_hashes_to_fetch, Vec::<AnyDhtHash>::new());

        // hash 1 is still present
        // hash 2 is missing now too
        // hash 2 should be only new hash
        let hash_2 = fixt!(AnyDhtHash);
        let filtered_hashes_to_fetch =
            validation_dependencies.get_new_hashes_to_fetch(vec![hash_1.clone(), hash_2.clone()]);
        assert_eq!(filtered_hashes_to_fetch, vec![hash_2.clone()]);

        // hash 1 has been fetched/removed in the meantime
        // hash 2 is still present
        // only hash 1 should be new
        validation_dependencies.remove_missing_hash(&hash_1);
        let filtered_hashes_to_fetch =
            validation_dependencies.get_new_hashes_to_fetch(vec![hash_1.clone(), hash_2.clone()]);
        assert_eq!(filtered_hashes_to_fetch, vec![hash_1.clone()]);
    }

    mod fetches_expiry_tests {
        use super::super::ValidationDependencies;
        use super::*;
        use std::time::Duration;

        #[test]
        fn empty() {
            let validation_dependencies = ValidationDependencies::default();
            assert_eq!(
                validation_dependencies.fetch_missing_hashes_timed_out(),
                true
            );
        }

        #[test]
        fn all_expired() {
            let mut validation_dependencies = ValidationDependencies::default();
            let hash = fixt!(AnyDhtHash);
            validation_dependencies.missing_hashes.insert(
                hash,
                Instant::now() - ValidationDependencies::FETCH_TIMEOUT - Duration::from_secs(1),
            );
            assert_eq!(
                validation_dependencies.fetch_missing_hashes_timed_out(),
                true
            );
        }

        #[test]
        fn none_expired() {
            let mut validation_dependencies = ValidationDependencies::default();
            let hash = fixt!(AnyDhtHash);
            validation_dependencies.missing_hashes.insert(
                hash,
                Instant::now() - ValidationDependencies::FETCH_TIMEOUT + Duration::from_secs(1),
            );
            assert_eq!(
                validation_dependencies.fetch_missing_hashes_timed_out(),
                false
            );
        }

        #[test]
        fn some_expired() {
            let mut validation_dependencies = ValidationDependencies::default();
            let unexpired_hash = fixt!(AnyDhtHash);
            let expired_hash = fixt!(AnyDhtHash);
            validation_dependencies.missing_hashes.insert(
                unexpired_hash,
                Instant::now() - ValidationDependencies::FETCH_TIMEOUT + Duration::from_secs(1),
            );
            validation_dependencies.missing_hashes.insert(
                expired_hash,
                Instant::now() - ValidationDependencies::FETCH_TIMEOUT - Duration::from_secs(1),
            );
            assert_eq!(
                validation_dependencies.fetch_missing_hashes_timed_out(),
                false
            );
        }
    }
}
