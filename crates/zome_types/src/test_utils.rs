//! Common helpers for writing tests against zome types
//!
//! We don't use fixturators for these, because this crate defines no fixturators

use crate::capability::CapSecret;
use crate::capability::CAP_SECRET_BYTES;
use crate::HostInput;
use holo_hash::{hash_type, *};
use holochain_serialized_bytes::prelude::*;

fn fake_holo_hash<T: holo_hash::HashType>(name: u8, hash_type: T) -> HoloHash<T> {
    HoloHash::from_raw_bytes_and_type([name; 36].to_vec(), hash_type)
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
    AgentPubKey::try_from("uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4").unwrap()
}

/// Another fixture AgentPubKey for unit testing.
/// NB: This must match up with AgentPubKeyFixturator's Predictable curve
pub fn fake_agent_pubkey_2() -> AgentPubKey {
    AgentPubKey::try_from("uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK").unwrap()
}

/// A fixture CapSecret for unit testing.
pub fn fake_cap_secret() -> CapSecret {
    [0; CAP_SECRET_BYTES].into()
}

/// A fixture ZomeCallInvocationPayload for unit testing.
pub fn fake_zome_invocation_payload() -> HostInput {
    HostInput::try_from(SerializedBytes::try_from(()).unwrap()).unwrap()
}
