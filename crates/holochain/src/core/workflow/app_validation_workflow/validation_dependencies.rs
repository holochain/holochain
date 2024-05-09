use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use holo_hash::{AnyDhtHash, DhtOpHash};
use holochain_types::dht_op::DhtOpHashed;

#[derive(Debug)]
pub(super) struct MissingHashProperties {
    depending_ops: HashSet<DhtOpHash>,
    when_fetched: Instant,
}

/// In-memory struct to keep track of missing DHT hashes, which DhtOp depends on them
/// and when they were started being fetched.
pub struct ValidationDependencies {
    /// Missing hashes that are being fetched, along with a set of DhtOps that depend
    /// on the hash and the timestamp a fetch was attempted.
    pub(super) missing_hashes: HashMap<AnyDhtHash, MissingHashProperties>,
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
        }
    }

    /// Returns true if this is a new missing hash.
    pub fn insert_missing_hash_for_op(&mut self, hash: AnyDhtHash, dht_op_hash: DhtOpHash) -> bool {
        if let std::collections::hash_map::Entry::Vacant(entry) =
            self.missing_hashes.entry(hash.clone())
        {
            let mut dht_op_hashes = HashSet::new();
            dht_op_hashes.insert(dht_op_hash);
            entry.insert(MissingHashProperties {
                depending_ops: dht_op_hashes,
                when_fetched: Instant::now(),
            });
            true
        } else {
            self.missing_hashes
                .entry(hash)
                .and_modify(|missing_hash_props| {
                    missing_hash_props.depending_ops.insert(dht_op_hash.clone());
                });
            false
        }
    }

    /// Iterates over given missing hashes and adds them to the internal collection if they are not present.
    /// A collection of new missing hashes is returned.
    /// Takes missing hashes and the depending op as parameters.
    pub fn insert_missing_hashes_for_op(
        &mut self,
        hashes: Vec<AnyDhtHash>,
        dht_op_hash: DhtOpHash,
    ) -> Vec<AnyDhtHash> {
        hashes
            .into_iter()
            .filter(|hash| self.insert_missing_hash_for_op(hash.clone(), dht_op_hash.clone()))
            .collect()
    }

    pub fn remove_missing_hash(&mut self, hash: &AnyDhtHash) {
        self.missing_hashes.remove(hash);
    }

    pub fn fetch_missing_hashes_timed_out(&self) -> bool {
        if self.missing_hashes.is_empty() {
            return false;
        }
        self.missing_hashes
            .iter()
            .all(|(_, MissingHashProperties { when_fetched, .. })| {
                when_fetched.elapsed() > Self::FETCH_TIMEOUT
            })
    }

    /// filter out dht_ops that have missing dependencies
    pub fn filter_ops_missing_dependencies(&self, dht_ops: Vec<DhtOpHashed>) -> Vec<DhtOpHashed> {
        dht_ops
            .into_iter()
            .filter(|dht_op| {
                self.missing_hashes.iter().all(
                    |(_, MissingHashProperties { depending_ops, .. })| {
                        !depending_ops.contains(&dht_op.hash)
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixt::fixt;
    use holo_hash::fixt::{AnyDhtHashFixturator, DhtOpHashFixturator};
    use holochain_types::dht_op::DhtOp;
    use holochain_zome_types::fixt::{ActionFixturator, SignatureFixturator};

    #[test]
    fn filter_missing_hashes_to_fetch_for_op() {
        let mut validation_dependencies = ValidationDependencies::default();

        let op = fixt!(DhtOpHash);
        // no missing hashes present
        // hash 1 should be only new hash
        let missing_hash_1 = fixt!(AnyDhtHash);
        let filtered_hashes_to_fetch = validation_dependencies
            .insert_missing_hashes_for_op(vec![missing_hash_1.clone()], op.clone());
        assert_eq!(filtered_hashes_to_fetch, [missing_hash_1.clone()].to_vec());

        // hash 1 is still present
        // new hashes should be empty
        let filtered_hashes_to_fetch = validation_dependencies
            .insert_missing_hashes_for_op(vec![missing_hash_1.clone()], op.clone());
        assert_eq!(filtered_hashes_to_fetch, Vec::<AnyDhtHash>::new());

        // hash 1 is still present
        // hash 2 is missing now too
        // hash 2 should be only new hash
        let missing_hash_2 = fixt!(AnyDhtHash);
        let filtered_hashes_to_fetch = validation_dependencies.insert_missing_hashes_for_op(
            vec![missing_hash_1.clone(), missing_hash_2.clone()],
            op.clone(),
        );
        assert_eq!(filtered_hashes_to_fetch, [missing_hash_2.clone()].to_vec());

        // hash 1 has been fetched/removed in the meantime
        // hash 2 is still present
        // only hash 1 should be new
        validation_dependencies.remove_missing_hash(&missing_hash_1);
        let filtered_hashes_to_fetch = validation_dependencies.insert_missing_hashes_for_op(
            vec![missing_hash_1.clone(), missing_hash_2.clone()],
            op.clone(),
        );
        assert_eq!(filtered_hashes_to_fetch, [missing_hash_1.clone()].to_vec());
    }

    #[test]
    fn filter_ops_missing_dependencies() {
        let mut validation_dependencies = ValidationDependencies::new();

        // op 1 is missing hashes
        // op 1 is the only hash to validate
        // filtered dht_ops should be empty
        let dht_op_1 = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));
        let missing_hash_1 = fixt!(AnyDhtHash);
        validation_dependencies
            .insert_missing_hash_for_op(missing_hash_1.clone(), dht_op_1.hash.clone());
        let dht_ops = vec![dht_op_1.clone()];
        let filtered_ops = validation_dependencies.filter_ops_missing_dependencies(dht_ops);
        assert_eq!(filtered_ops, Vec::<DhtOpHashed>::new());

        // op 1 misses another hash
        // op 1 still the only hash to validate
        // filtered dht_ops should be empty again
        let missing_hash_2 = fixt!(AnyDhtHash);
        validation_dependencies
            .insert_missing_hash_for_op(missing_hash_2.clone(), dht_op_1.hash.clone());
        let dht_ops = vec![dht_op_1.clone()];
        let filtered_ops = validation_dependencies.filter_ops_missing_dependencies(dht_ops);
        assert_eq!(filtered_ops, Vec::<DhtOpHashed>::new());

        // op 2 is new to validate
        // op 1 still to validate
        // filtered dht_ops should only contain op 2
        let dht_op_2 = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));
        let dht_ops = vec![dht_op_1.clone(), dht_op_2.clone()];
        let filtered_ops = validation_dependencies.filter_ops_missing_dependencies(dht_ops);
        assert_eq!(filtered_ops, vec![dht_op_2.clone()]);

        // op 1's missing hash has been fetched, but it still has another missing hash
        // op 2 is no longer validated
        // op 3 is validated
        // filtered dht_ops should only contain op 3
        validation_dependencies.remove_missing_hash(&missing_hash_1);
        let dht_op_3 = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));
        let dht_ops = vec![dht_op_1.clone(), dht_op_3.clone()];
        let filtered_ops = validation_dependencies.filter_ops_missing_dependencies(dht_ops);
        assert_eq!(filtered_ops, vec![dht_op_3.clone()]);

        // all missing hashes fetched
        // op 4 and 5 to be validated
        // filtered dht_ops should contain op 4 and 5
        validation_dependencies.remove_missing_hash(&missing_hash_2);
        let dht_op_4 = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));
        let dht_op_5 = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));
        let dht_ops = vec![dht_op_4.clone(), dht_op_5.clone()];
        let filtered_ops = validation_dependencies.filter_ops_missing_dependencies(dht_ops);
        assert_eq!(filtered_ops, vec![dht_op_4, dht_op_5]);
    }

    mod fetches_expiration {
        use super::*;

        #[test]
        fn empty() {
            let validation_dependencies = ValidationDependencies::default();
            assert_eq!(
                validation_dependencies.fetch_missing_hashes_timed_out(),
                false
            );
        }

        #[test]
        fn all_expired() {
            let mut validation_dependencies = ValidationDependencies::default();
            let hash = fixt!(AnyDhtHash);
            validation_dependencies.missing_hashes.insert(
                hash,
                MissingHashProperties {
                    depending_ops: HashSet::new(),
                    // 1 second longer than fetch timeout
                    when_fetched: Instant::now()
                        - ValidationDependencies::FETCH_TIMEOUT
                        - Duration::from_secs(1),
                },
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
            // 1 second before than fetch timeout
            validation_dependencies.missing_hashes.insert(
                hash,
                MissingHashProperties {
                    depending_ops: HashSet::new(),
                    when_fetched: Instant::now() - ValidationDependencies::FETCH_TIMEOUT
                        + Duration::from_secs(1),
                },
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
                MissingHashProperties {
                    depending_ops: HashSet::new(),
                    when_fetched: Instant::now() - ValidationDependencies::FETCH_TIMEOUT
                        + Duration::from_secs(1),
                },
            );
            validation_dependencies.missing_hashes.insert(
                expired_hash,
                MissingHashProperties {
                    depending_ops: HashSet::new(),
                    when_fetched: Instant::now()
                        - ValidationDependencies::FETCH_TIMEOUT
                        - Duration::from_secs(1),
                },
            );
            assert_eq!(
                validation_dependencies.fetch_missing_hashes_timed_out(),
                false
            );
        }
    }
}
