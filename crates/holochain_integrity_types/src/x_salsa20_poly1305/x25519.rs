use holochain_secure_primitive::secure_primitive;
use holochain_serialized_bytes::prelude::*;
use std::hash::{Hash, Hasher};
use ts_rs::TS;
use export_types_config::EXPORT_TS_TYPES_FILE;

pub const X25519_PUB_KEY_BYTES: usize = 32;

#[derive(Clone, Copy, SerializedBytes, TS)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub struct X25519PubKey([u8; X25519_PUB_KEY_BYTES]);

secure_primitive!(X25519PubKey, X25519_PUB_KEY_BYTES);

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for X25519PubKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn calculate_hash(key: &X25519PubKey) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn x25519_pubkey_hash_consistency() {
        let key_bytes = [42u8; X25519_PUB_KEY_BYTES];
        let pubkey: X25519PubKey = key_bytes.into();

        // Hash should be consistent across multiple calls
        let hash1 = calculate_hash(&pubkey);
        let hash2 = calculate_hash(&pubkey);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn x25519_pubkey_hash_uniqueness() {
        let key_bytes1 = [1u8; X25519_PUB_KEY_BYTES];
        let key_bytes2 = [2u8; X25519_PUB_KEY_BYTES];

        let pubkey1: X25519PubKey = key_bytes1.into();
        let pubkey2: X25519PubKey = key_bytes2.into();

        // Different keys should have different hashes
        let hash1 = calculate_hash(&pubkey1);
        let hash2 = calculate_hash(&pubkey2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn x25519_pubkey_hash_works_with_hashset() {
        let mut set = HashSet::new();

        let key_bytes1 = [1u8; X25519_PUB_KEY_BYTES];
        let key_bytes2 = [2u8; X25519_PUB_KEY_BYTES];
        let key_bytes3 = [1u8; X25519_PUB_KEY_BYTES]; // Same as first

        let pubkey1: X25519PubKey = key_bytes1.into();
        let pubkey2: X25519PubKey = key_bytes2.into();
        let pubkey3: X25519PubKey = key_bytes3.into();

        // Insert keys into HashSet
        assert!(set.insert(pubkey1));
        assert!(set.insert(pubkey2));
        assert!(!set.insert(pubkey3)); // Should not insert since it's the same as pubkey1

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn x25519_pubkey_hash_based_on_bytes() {
        let key_bytes = [123u8; X25519_PUB_KEY_BYTES];
        let pubkey: X25519PubKey = key_bytes.into();

        // Hash of pubkey should be the same as hash of its internal bytes
        let pubkey_hash = calculate_hash(&pubkey);

        let bytes_hash = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            key_bytes.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(pubkey_hash, bytes_hash);
    }
}
