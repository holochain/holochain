//! Read operations on the per-DNA DHT store.
//!
//! Methods on [`DhtStoreRead`] expose domain-meaningful reads for the
//! holochain crate's workflows. They delegate to `holochain_data`'s
//! `DbRead<Dht>` primitives and return values in terms of the project's
//! existing domain types.

use super::{DhtStore, DhtStoreRead};
use crate::query::StateQueryResult;
use holo_hash::{DhtOpHash, HasHash};
use holochain_data::kind::Dht;
use holochain_data::{DbRead, DbWrite};
use holochain_types::dht_op::DhtOpHashed;

impl DhtStore<DbRead<Dht>> {
    /// Returns `true` if `hash` appears in any op-bearing DHT table
    /// (`ChainOp`, `LimboChainOp`, `Warrant`, `LimboWarrant`).
    pub async fn op_exists(&self, hash: &DhtOpHash) -> StateQueryResult<bool> {
        Ok(self.db().op_exists(hash).await?)
    }

    /// Drop any op whose hash is already recorded in the DHT store.
    /// Input order is preserved for surviving ops.
    pub async fn filter_existing_ops(
        &self,
        ops: Vec<DhtOpHashed>,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        let hashes: Vec<DhtOpHash> = ops.iter().map(|o| o.as_hash().clone()).collect();
        let present = self.db().op_hashes_present(&hashes).await?;
        Ok(ops
            .into_iter()
            .zip(present)
            .filter_map(|(op, exists)| if exists { None } else { Some(op) })
            .collect())
    }
}

impl DhtStore<DbWrite<Dht>> {
    /// Returns `true` if `hash` appears in any op-bearing DHT table.
    ///
    /// Delegates to the read-only view of this store.
    pub async fn op_exists(&self, hash: &DhtOpHash) -> StateQueryResult<bool> {
        self.as_read().op_exists(hash).await
    }

    /// Drop any op whose hash is already recorded in the DHT store.
    ///
    /// Delegates to the read-only view of this store.
    pub async fn filter_existing_ops(
        &self,
        ops: Vec<DhtOpHashed>,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        self.as_read().filter_existing_ops(ops).await
    }
}

// Compile-only sanity check that the read-only alias resolves correctly.
#[allow(dead_code)]
fn _read_only_alias_compiles(_: DhtStoreRead) {}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, EntryHash, HoloHashed};
    use holochain_data::kind::Dht;
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_types::prelude::Signature;
    use holochain_types::prelude::Timestamp;
    use holochain_zome_types::action::{Action, Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::AppEntryDef;
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(holo_hash::DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn make_chain_op(seed: u8) -> DhtOpHashed {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let action = Action::Create(Create {
            author,
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: 1,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]),
            weight: Default::default(),
        });
        let chain_op = ChainOp::RegisterAgentActivity(Signature::from([seed; 64]), action);
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op)))
    }

    /// Build a synthetic `DhtOpHashed` with the given pre-computed hash.
    fn make_chain_op_with_hash(seed: u8, hash: DhtOpHash) -> DhtOpHashed {
        let op = make_chain_op(seed);
        HoloHashed::with_pre_hashed(op.into_inner().0, hash)
    }

    #[tokio::test]
    async fn op_exists_returns_false_for_unknown_hash() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let unknown = DhtOpHash::from_raw_36(vec![99u8; 36]);
        let exists = store.op_exists(&unknown).await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn op_exists_returns_true_after_record_incoming_ops() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(1);
        let hash = op.as_hash().clone();

        store.record_incoming_ops(vec![op]).await.unwrap();

        let exists = store.op_exists(&hash).await.unwrap();
        assert!(exists, "op_exists should be true after record_incoming_ops");
    }

    #[tokio::test]
    async fn filter_existing_ops_removes_known_hashes() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let known = make_chain_op(2);
        let unknown = make_chain_op(3);
        let known_hash = known.as_hash().clone();
        let unknown_hash = unknown.as_hash().clone();

        store.record_incoming_ops(vec![known]).await.unwrap();

        let input = vec![make_chain_op_with_hash(20, known_hash.clone()), unknown];
        let filtered = store.as_read().filter_existing_ops(input).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].as_hash(), &unknown_hash);
    }
}
