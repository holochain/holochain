//! Per-author source-chain write permit for the DHT database.

use crate::handles::DbWrite;
use crate::kind::Dht;
use crate::DatabaseIdentifier;
use holo_hash::AgentPubKey;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

impl DbWrite<Dht> {
    /// Acquire the per-author source-chain write permit for this DHT database.
    ///
    /// Source-chain flushes for a single `(DNA, author)` chain must be
    /// serialized: two concurrent flushes that both read the same chain head,
    /// pass their as-at check, and then write would fork the chain. Holding
    /// this permit across the as-at check and the source-chain write closes
    /// that window.
    ///
    /// Each `(DNA, author)` pair has an independent single-permit semaphore, so
    /// different authors — and the same author on different DNAs — never block
    /// one another. The DNA is identified by the database's stable
    /// [`database_id`](crate::DatabaseIdentifier::database_id) (the DHT
    /// database is per-DNA), combined with the author key. This mirrors the
    /// granularity of the legacy `holochain_sqlite` authored-DB write permit,
    /// which was keyed by the per-cell `DbKind::Authored(CellId)`.
    pub async fn acquire_chain_write_permit(&self, author: &AgentPubKey) -> OwnedSemaphorePermit {
        let key = (self.identifier().database_id().to_string(), author.clone());
        chain_write_semaphore(key)
            .acquire_owned()
            .await
            .expect("chain write semaphore is never closed")
    }
}

/// Fetch (or lazily create) the single-permit semaphore for a
/// `(database_id, author)` key.
///
/// Mirrors the keyed-`Lazy`-static semaphore pattern used by the legacy
/// `holochain_sqlite` write permit, but keyed per author so each chain
/// serializes independently. The map is never pruned; the number of live keys
/// is bounded by the installed `(DNA, author)` cells, matching the legacy
/// per-cell authored-DB semaphore map.
fn chain_write_semaphore(key: (String, AgentPubKey)) -> Arc<Semaphore> {
    type SemaphoreMap = HashMap<(String, AgentPubKey), Arc<Semaphore>>;
    static MAP: LazyLock<Mutex<SemaphoreMap>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    MAP.lock()
        .expect("chain write semaphore map is not poisoned")
        .entry(key)
        .or_insert_with(|| Arc::new(Semaphore::new(1)))
        .clone()
}

#[cfg(test)]
mod tests {
    use crate::kind::Dht;
    use crate::test_open_db;
    use holo_hash::{AgentPubKey, DnaHash};
    use std::sync::Arc;
    use std::time::Duration;

    fn dht_db_id(seed: u8) -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![seed; 36])))
    }

    /// Two acquisitions for the same `(DNA, author)` chain must not overlap: the
    /// second blocks until the first permit is dropped.
    #[tokio::test]
    async fn same_author_serializes() {
        let db = test_open_db(dht_db_id(0)).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);

        let permit1 = db.acquire_chain_write_permit(&author).await;

        let db2 = db.clone();
        let author2 = author.clone();
        let handle = tokio::spawn(async move {
            let _p = db2.acquire_chain_write_permit(&author2).await;
        });

        // Give the spawned task a chance to run; it must still be blocked while
        // we hold the first permit.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !handle.is_finished(),
            "second acquire for the same author must block while the first permit is held"
        );

        drop(permit1);

        // Once the first permit is released the second acquire completes.
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("second acquire should complete after the first permit is dropped")
            .unwrap();
    }

    /// Different authors on the same DHT database use independent semaphores and
    /// do not block one another.
    #[tokio::test]
    async fn different_authors_do_not_block() {
        let db = test_open_db(dht_db_id(0)).await.unwrap();
        let alice = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let bob = AgentPubKey::from_raw_36(vec![2u8; 36]);

        let _alice_permit = db.acquire_chain_write_permit(&alice).await;

        let _bob_permit =
            tokio::time::timeout(Duration::from_secs(1), db.acquire_chain_write_permit(&bob))
                .await
                .expect("a different author must not be blocked by alice's permit");
    }

    /// The same author on different DHT databases (different DNAs) uses
    /// independent semaphores and does not block.
    #[tokio::test]
    async fn same_author_different_dna_do_not_block() {
        let db_a = test_open_db(dht_db_id(0)).await.unwrap();
        let db_b = test_open_db(dht_db_id(9)).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);

        let _permit_a = db_a.acquire_chain_write_permit(&author).await;

        let _permit_b = tokio::time::timeout(
            Duration::from_secs(1),
            db_b.acquire_chain_write_permit(&author),
        )
        .await
        .expect("the same author on a different DNA must not be blocked");
    }
}
