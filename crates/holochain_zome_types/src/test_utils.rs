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
    HoloHash::from_raw_32_and_type([name; 32].to_vec(), hash_type)
}

/// A fixture DnaHash for unit testing.
pub fn fake_dna_hash(name: u8) -> DnaHash {
    fake_holo_hash(name, hash_type::Dna::new())
}

/// A fixture ActionHash for unit testing.
pub fn fake_action_hash(name: u8) -> ActionHash {
    fake_holo_hash(name, hash_type::Action::new())
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
    AgentPubKey::try_from("uhCAkJCuynkgVdMn_bzZ2ZYaVfygkn0WCuzfFspczxFnZM1QAyXoo").unwrap()
}

/// Another fixture AgentPubKey for unit testing.
/// NB: This must match up with AgentPubKeyFixturator's Predictable curve
pub fn fake_agent_pubkey_2() -> AgentPubKey {
    AgentPubKey::try_from("uhCAk39SDf7rynCg5bYgzroGaOJKGKrloI1o57Xao6S-U5KNZ0dUH").unwrap()
}

/// A fixture CapSecret for unit testing.
pub fn fake_cap_secret() -> CapSecret {
    [0; CAP_SECRET_BYTES].into()
}

/// A fixture example CellId for unit testing.
pub fn fake_cell_id(name: u8) -> CellId {
    (fake_dna_hash(name), fake_agent_pubkey_1()).into()
}
