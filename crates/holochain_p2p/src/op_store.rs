//! Kitsune2 [`OpStore`] backed by the `holochain_data` per-DNA DHT
//! database via `holochain_state::DhtStore`.
//!
//! All reads go through `DhtStore` methods; this module is responsible only
//! for marshalling K2 types (`OpId`, `MetaOp`, `DhtArc`, `Timestamp`) into
//! and out of the row shapes returned by the store. Wire bytes for chain
//! ops are built by converting the stored v2 `SignedAction` back to the
//! legacy form via `holochain_zome_types::dht_v2::to_legacy_signed_action`
//! and encoding the resulting `DhtOp`.

use bytes::{Bytes, BytesMut};
use futures::future::BoxFuture;
use holo_hash::{DhtOpHash, DnaHash, HoloHashed, HOLO_HASH_CORE_LEN, HOLO_HASH_UNTYPED_LEN};
use holochain_serialized_bytes::prelude::{decode, encode};
use holochain_state::dht_store::{K2ChainOpForWireRow, K2WarrantForWireRow};
use holochain_state::DhtStore;
use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
use holochain_types::warrant::WarrantOp;
use holochain_zome_types::dht_v2::{
    to_legacy_signed_action, Action as ActionV2, ActionData, ActionHeader, SignedActionHashed,
};
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::warrant::{SignedWarrant, Warrant, WarrantProof};
use kitsune2_api::*;
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

/// Holochain implementation of the Kitsune2 [OpStoreFactory].
pub struct HolochainOpStoreFactory {
    /// Returns the `DhtStore` for a DNA. The store is write-capable so it
    /// can also handle `store_slice_hash`; reads are exposed through the
    /// same handle.
    pub getter: crate::GetDhtStore,
    /// The event handler.
    pub handler: Arc<std::sync::OnceLock<crate::spawn::WrapEvtSender>>,
}

impl std::fmt::Debug for HolochainOpStoreFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainOpStoreFactory").finish()
    }
}

impl kitsune2_api::OpStoreFactory for HolochainOpStoreFactory {
    fn default_config(&self, _config: &mut kitsune2_api::Config) -> kitsune2_api::K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &kitsune2_api::Config) -> kitsune2_api::K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        _builder: Arc<kitsune2_api::Builder>,
        space: kitsune2_api::SpaceId,
    ) -> BoxFut<'static, kitsune2_api::K2Result<kitsune2_api::DynOpStore>> {
        let getter = self.getter.clone();
        let handler = self.handler.clone();
        Box::pin(async move {
            let dna_hash = DnaHash::from_k2_space(&space);
            let store = getter(dna_hash.clone())
                .await
                .map_err(|err| kitsune2_api::K2Error::other_src("failed to get dht store", err))?;
            let op_store: kitsune2_api::DynOpStore =
                Arc::new(HolochainOpStore::new(store, dna_hash, handler));
            Ok(op_store)
        })
    }
}

/// Holochain implementation of the Kitsune2 [OpStore].
pub struct HolochainOpStore {
    store: DhtStore,
    dna_hash: DnaHash,
    sender: Arc<std::sync::OnceLock<crate::spawn::WrapEvtSender>>,
}

impl Debug for HolochainOpStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainOpStore")
            .field("dna_hash", &self.dna_hash)
            .finish()
    }
}

impl HolochainOpStore {
    /// Create a new [HolochainOpStore].
    pub fn new(
        store: DhtStore,
        dna_hash: DnaHash,
        sender: Arc<std::sync::OnceLock<crate::spawn::WrapEvtSender>>,
    ) -> HolochainOpStore {
        Self {
            store,
            dna_hash,
            sender,
        }
    }
}

/// Build a K2 located [`OpId`] from the raw 36-byte op-hash and basis-hash
/// blobs stored in the DHT database (no type prefix in either).
///
/// `to_located_k2_op_id` on `HoloHash` would do this too, but it requires
/// typed `DhtOpHash` / `OpBasis` values, and `OpBasis = AnyLinkableHash`
/// can't be reconstructed from a 36-byte blob alone (the type prefix has
/// been stripped). K2 only needs the op-hash core + the basis location
/// bytes, both of which are present in the stored 36-byte forms.
fn k2_op_id_from_raw(op_hash_36: &[u8], basis_36: &[u8]) -> OpId {
    debug_assert_eq!(op_hash_36.len(), HOLO_HASH_UNTYPED_LEN);
    debug_assert_eq!(basis_36.len(), HOLO_HASH_UNTYPED_LEN);
    let mut inner = BytesMut::with_capacity(HOLO_HASH_UNTYPED_LEN);
    inner.extend_from_slice(&op_hash_36[..HOLO_HASH_CORE_LEN]);
    inner.extend_from_slice(&basis_36[HOLO_HASH_CORE_LEN..]);
    OpId::from(inner.freeze())
}

impl OpStore for HolochainOpStore {
    fn process_incoming_ops(&self, op_list: Vec<Bytes>) -> BoxFut<'_, K2Result<Vec<OpId>>> {
        Box::pin(async move {
            let mut dht_ops = Vec::with_capacity(op_list.len());
            let mut ids = Vec::with_capacity(op_list.len());
            for op_bytes in op_list {
                let op = decode::<_, DhtOp>(&op_bytes)
                    .map_err(|e| K2Error::other_src("Could not decode op", e))?;
                let op_hashed = DhtOpHashed::from_content_sync(op.clone());
                ids.push(op_hashed.hash.to_located_k2_op_id(&op.dht_basis()));
                dht_ops.push(op);
            }

            use crate::types::event::HcP2pHandler;
            self.sender
                .get()
                .ok_or_else(|| K2Error::other("event handler not registered"))?
                .handle_publish(self.dna_hash.clone(), dht_ops)
                .await
                .map_err(|e| K2Error::other_src("Failed to publish incoming ops", e))?;

            Ok(ids)
        })
    }

    fn retrieve_op_hashes_in_time_slice(
        &self,
        arc: DhtArc,
        start: Timestamp,
        end: Timestamp,
    ) -> BoxFuture<'_, K2Result<(Vec<OpId>, u32)>> {
        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => return Box::pin(async move { Ok((vec![], 0)) }),
            DhtArc::Arc(start, end) => (start, end),
        };
        let start_t = holochain_timestamp::Timestamp::from_micros(start.as_micros());
        let end_t = holochain_timestamp::Timestamp::from_micros(end.as_micros());
        Box::pin(async move {
            let rows = self
                .store
                .k2_op_hashes_in_time_slice(arc_start, arc_end, start_t, end_t)
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve op hashes in time slice", e))?;
            let mut out = Vec::with_capacity(rows.len());
            let mut total: u32 = 0;
            for row in rows {
                out.push(k2_op_id_from_raw(&row.hash, &row.basis_hash));
                total = total.saturating_add(row.serialized_size.max(0) as u32);
            }
            Ok((out, total))
        })
    }

    fn retrieve_ops(&self, op_ids: Vec<OpId>) -> BoxFuture<'_, K2Result<Vec<MetaOp>>> {
        // Convert K2 op-ids back to raw hash bytes; skip and log invalid ones
        // rather than aborting the whole batch.
        let raw_hashes: Vec<Vec<u8>> = op_ids
            .iter()
            .filter_map(|id| match DhtOpHash::try_from_k2_op(id) {
                Ok(h) => Some(h.get_raw_36().to_vec()),
                Err(e) => {
                    tracing::warn!("Cannot retrieve op for invalid op id: {e}");
                    None
                }
            })
            .collect();

        Box::pin(async move {
            if raw_hashes.is_empty() {
                return Ok(Vec::new());
            }
            let chain_rows = self
                .store
                .k2_get_chain_ops_for_wire(&raw_hashes)
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve chain ops", e))?;
            let warrant_rows = self
                .store
                .k2_get_warrants_for_wire(&raw_hashes)
                .await
                .map_err(|e| K2Error::other_src("Failed to retrieve warrants", e))?;

            let mut out = Vec::with_capacity(chain_rows.len() + warrant_rows.len());

            for row in chain_rows {
                let op = match build_chain_dht_op(row) {
                    Ok(op) => op,
                    Err(e) => {
                        tracing::warn!("Failed to reconstruct chain op for wire: {e}");
                        continue;
                    }
                };
                let dht_op_hash = DhtOpHashed::from_content_sync(op.clone()).hash;
                let op_id = dht_op_hash.to_located_k2_op_id(&op.dht_basis());
                let op_data =
                    encode(&op).map_err(|e| K2Error::other_src("Failed to encode chain op", e))?;
                out.push(MetaOp {
                    op_id,
                    op_data: op_data.into(),
                });
            }

            for row in warrant_rows {
                let op = match build_warrant_dht_op(row) {
                    Ok(op) => op,
                    Err(e) => {
                        tracing::warn!("Failed to reconstruct warrant op for wire: {e}");
                        continue;
                    }
                };
                let dht_op_hash = DhtOpHashed::from_content_sync(op.clone()).hash;
                let op_id = dht_op_hash.to_located_k2_op_id(&op.dht_basis());
                let op_data = encode(&op)
                    .map_err(|e| K2Error::other_src("Failed to encode warrant op", e))?;
                out.push(MetaOp {
                    op_id,
                    op_data: op_data.into(),
                });
            }

            Ok(out)
        })
    }

    fn filter_out_existing_ops(&self, op_ids: Vec<OpId>) -> BoxFuture<'_, K2Result<Vec<OpId>>> {
        // Build the candidate set + hash-byte lookup; skip invalid op-ids.
        let mut candidate_set: HashSet<OpId> = HashSet::with_capacity(op_ids.len());
        let mut raw_hashes: Vec<Vec<u8>> = Vec::with_capacity(op_ids.len());
        for id in &op_ids {
            match DhtOpHash::try_from_k2_op(id) {
                Ok(h) => {
                    raw_hashes.push(h.get_raw_36().to_vec());
                    candidate_set.insert(id.clone());
                }
                Err(e) => {
                    tracing::warn!("Got invalid op id: {e}");
                }
            }
        }
        Box::pin(async move {
            if raw_hashes.is_empty() {
                return Ok(Vec::new());
            }
            let rows = self
                .store
                .k2_check_op_hashes_present(&raw_hashes)
                .await
                .map_err(|e| K2Error::other_src("Failed to filter out existing ops", e))?;
            for row in rows {
                candidate_set.remove(&k2_op_id_from_raw(&row.hash, &row.basis_hash));
            }
            Ok(candidate_set.into_iter().collect())
        })
    }

    fn retrieve_op_ids_bounded(
        &self,
        arc: DhtArc,
        start: Timestamp,
        limit_bytes: u32,
    ) -> BoxFuture<'_, K2Result<(Vec<OpId>, u32, Timestamp)>> {
        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => return Box::pin(async move { Ok((vec![], 0, start)) }),
            DhtArc::Arc(start, end) => (start, end),
        };

        Box::pin(async move {
            let mut used_bytes: u32 = 0;
            let mut latest_timestamp = start;
            let mut out: HashSet<OpId> = HashSet::new();

            // Page in batches of 500 to bound memory while still observing
            // `limit_bytes`. The integration timestamp is monotonic per
            // local clock; >500 ops at the exact same micro is implausible
            // so the cursor always advances.
            'outer: loop {
                let cursor_t =
                    holochain_timestamp::Timestamp::from_micros(latest_timestamp.as_micros());
                let rows = self
                    .store
                    .k2_op_ids_since_time_batch(arc_start, arc_end, cursor_t, 500)
                    .await
                    .map_err(|e| K2Error::other_src("Failed to retrieve op ids bounded", e))?;
                let ops_size_before = out.len();
                for row in rows {
                    let row_size = row.serialized_size.max(0) as u32;
                    if used_bytes.saturating_add(row_size) > limit_bytes {
                        break 'outer;
                    }
                    let op_id = k2_op_id_from_raw(&row.hash, &row.basis_hash);
                    if out.insert(op_id) {
                        latest_timestamp = Timestamp::from_micros(row.when_integrated);
                        used_bytes = used_bytes.saturating_add(row_size);
                    }
                }
                if out.len() == ops_size_before {
                    break;
                }
            }

            Ok((out.into_iter().collect(), used_bytes, latest_timestamp))
        })
    }

    fn earliest_timestamp_in_arc(&self, arc: DhtArc) -> BoxFuture<'_, K2Result<Option<Timestamp>>> {
        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => return Box::pin(async move { Ok(None) }),
            DhtArc::Arc(start, end) => (start, end),
        };
        Box::pin(async move {
            let opt = self
                .store
                .k2_earliest_authored_timestamp_in_arc(arc_start, arc_end)
                .await
                .map_err(|e| {
                    K2Error::other_src("Failed to retrieve earliest timestamp in arc", e)
                })?;
            Ok(opt.map(|t| Timestamp::from_micros(t.as_micros())))
        })
    }

    fn query_total_op_count(&self) -> BoxFuture<'_, K2Result<u64>> {
        Box::pin(async move {
            self.store
                .k2_total_integrated_op_count()
                .await
                .map_err(|e| K2Error::other_src("Failed to query total op count", e))
        })
    }

    fn store_slice_hash(
        &self,
        arc: DhtArc,
        slice_index: u64,
        slice_hash: Bytes,
    ) -> BoxFuture<'_, K2Result<()>> {
        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => return Box::pin(async move { Ok(()) }),
            DhtArc::Arc(start, end) => (start, end),
        };
        let bytes = slice_hash.to_vec();
        Box::pin(async move {
            self.store
                .store_slice_hash(arc_start, arc_end, slice_index, &bytes)
                .await
                .map_err(|e| K2Error::other_src("Failed to store slice hash", e))
        })
    }

    fn slice_hash_count(&self, arc: DhtArc) -> BoxFuture<'_, K2Result<u64>> {
        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => return Box::pin(async move { Ok(0) }),
            DhtArc::Arc(start, end) => (start, end),
        };
        Box::pin(async move {
            self.store
                .slice_hash_count(arc_start, arc_end)
                .await
                .map_err(|e| K2Error::other_src("Failed to count slice hashes", e))
        })
    }

    fn retrieve_slice_hash(
        &self,
        arc: DhtArc,
        slice_index: u64,
    ) -> BoxFuture<'_, K2Result<Option<Bytes>>> {
        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => return Box::pin(async move { Ok(None) }),
            DhtArc::Arc(start, end) => (start, end),
        };
        Box::pin(async move {
            self.store
                .get_slice_hash(arc_start, arc_end, slice_index)
                .await
                .map(|opt| opt.map(Bytes::from))
                .map_err(|e| K2Error::other_src("Failed to retrieve slice hash", e))
        })
    }

    fn retrieve_slice_hashes(&self, arc: DhtArc) -> BoxFuture<'_, K2Result<Vec<(u64, Bytes)>>> {
        let (arc_start, arc_end) = match arc {
            DhtArc::Empty => return Box::pin(async move { Ok(vec![]) }),
            DhtArc::Arc(start, end) => (start, end),
        };
        Box::pin(async move {
            self.store
                .get_slice_hashes(arc_start, arc_end)
                .await
                .map(|rows| {
                    rows.into_iter()
                        .map(|r| (r.slice_index.max(0) as u64, Bytes::from(r.hash)))
                        .collect()
                })
                .map_err(|e| K2Error::other_src("Failed to retrieve slice hashes", e))
        })
    }
}

/// Reconstruct a legacy [`DhtOp::ChainOp`] from the joined row.
fn build_chain_dht_op(row: K2ChainOpForWireRow) -> Result<DhtOp, String> {
    use holo_hash::{ActionHash, AgentPubKey};

    let op_type_i: i64 = row.op_type;
    let op_type: ChainOpType = ChainOpType::try_from(op_type_i)
        .map_err(|v| format!("invalid op_type {v} in ChainOp row"))?;

    let action_data: ActionData = holochain_serialized_bytes::decode(&row.action_data)
        .map_err(|e| format!("failed to decode ActionData: {e:?}"))?;
    let action_hash = ActionHash::from_raw_36(row.action_hash);
    let prev_action = row.prev_hash.map(ActionHash::from_raw_36);

    let header = ActionHeader {
        author: AgentPubKey::from_raw_36(row.author),
        timestamp: holochain_timestamp::Timestamp::from_micros(row.timestamp),
        action_seq: row.seq.max(0) as u32,
        prev_action,
    };
    let v2_action = ActionV2 {
        header,
        data: action_data,
    };
    let signature = decode_signature(&row.signature)?;

    let sah: SignedActionHashed = holochain_zome_types::record::SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(v2_action, action_hash),
        signature,
    );
    let legacy_sah = to_legacy_signed_action(&sah);

    let entry = match row.entry_blob {
        Some(blob) => Some(
            holochain_serialized_bytes::decode::<_, holochain_types::prelude::Entry>(&blob)
                .map_err(|e| format!("failed to decode Entry blob: {e:?}"))?,
        ),
        None => None,
    };

    let chain_op = ChainOp::from_type(op_type, legacy_sah.into(), entry)
        .map_err(|e| format!("failed to build legacy ChainOp: {e:?}"))?;
    Ok(DhtOp::ChainOp(Box::new(chain_op)))
}

/// Reconstruct a legacy [`DhtOp::WarrantOp`] from the warrant row.
fn build_warrant_dht_op(row: K2WarrantForWireRow) -> Result<DhtOp, String> {
    use holo_hash::AgentPubKey;

    let author = AgentPubKey::from_raw_36(row.author);
    let warrantee = AgentPubKey::from_raw_36(row.warrantee);
    let timestamp = holochain_timestamp::Timestamp::from_micros(row.timestamp);
    let proof: WarrantProof = holochain_serialized_bytes::decode(&row.proof)
        .map_err(|e| format!("failed to decode WarrantProof: {e:?}"))?;
    let signature = decode_signature(&row.signature)?;

    let warrant = Warrant {
        proof,
        warrantee,
        author,
        timestamp,
    };
    let signed = SignedWarrant::new(warrant, signature);
    Ok(DhtOp::WarrantOp(Box::new(WarrantOp::from(signed))))
}

fn decode_signature(bytes: &[u8]) -> Result<holochain_zome_types::signature::Signature, String> {
    let arr: [u8; 64] = bytes
        .try_into()
        .map_err(|_| format!("signature length {} is not 64 bytes", bytes.len()))?;
    Ok(holochain_zome_types::signature::Signature::from(arr))
}
