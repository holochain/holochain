//! Common helpers for writing tests against zome types
//!
//! We don't use fixturators for these, because this crate defines no fixturators

use crate::capability::CapSecret;
use crate::capability::CAP_SECRET_BYTES;
use crate::cell::CellId;
use holo_hash::hash_type;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

fn fake_holo_hash<T: holo_hash::HashType>(name: u8, hash_type: T) -> HoloHash<T> {
    HoloHash::from_raw_36_and_type([name; HOLO_HASH_UNTYPED_LEN].to_vec(), hash_type)
}

/// A fixture DnaHash for unit testing.
pub fn fake_dna_hash(name: u8) -> DnaHash {
    fake_holo_hash(name, hash_type::Dna::new())
}

/// A fixture HeaderHash for unit testing.
pub fn fake_header_hash(name: u8) -> HeaderHash {
    fake_holo_hash(name, hash_type::Header::new())
}

/// A fixture DhtOpHash for unit testing.
pub fn fake_dht_op_hash(name: u8) -> DhtOpHash {
    fake_holo_hash(name, hash_type::DhtOp::new())
}

/// A fixture EntryHash for unit testing.
pub fn fake_entry_hash(name: u8) -> EntryHash {
    fake_holo_hash(name, hash_type::Entry::new())
}

/// A fixture AgentPubKey for unit testing.
pub fn fake_agent_pub_key(name: u8) -> AgentPubKey {
    fake_holo_hash(name, hash_type::Agent::new())
}

/// A fixture AgentPubKey for unit testing.
/// NB: This must match up with AgentPubKeyFixturator's Predictable curve
pub fn fake_agent_pubkey_1() -> AgentPubKey {
    AgentPubKey::try_from("uhCAkmrkoAHPVf_eufG7eC5fm6QKrW5pPMoktvG5LOC0SnJ4vV1Uv").unwrap()
}

/// Another fixture AgentPubKey for unit testing.
/// NB: This must match up with AgentPubKeyFixturator's Predictable curve
pub fn fake_agent_pubkey_2() -> AgentPubKey {
    AgentPubKey::try_from("uhCAke1j8Z2a-_min0h0pGuEMcYlo_V1l1mt9OtBuywKmHlg4L_R-").unwrap()
}

/// A fixture CapSecret for unit testing.
pub fn fake_cap_secret() -> CapSecret {
    [0; CAP_SECRET_BYTES].into()
}

/// A fixture example CellId for unit testing.
pub fn fake_cell_id(name: u8) -> CellId {
    (fake_dna_hash(name), fake_agent_pubkey_1()).into()
}
