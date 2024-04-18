mod fetches_expiry_tests {
    use ::fixt::fixt;
    use holo_hash::fixt::AnyDhtHashFixturator;
    use std::time::{Duration, Instant};

    use crate::core::workflow::app_validation_workflow::ValidationDependencies;

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
