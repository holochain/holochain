pub use crate::app_entry;
pub use crate::capability::create_cap_claim;
pub use crate::capability::create_cap_grant;
pub use crate::capability::delete_cap_grant;
pub use crate::capability::generate_cap_secret;
pub use crate::capability::update_cap_grant;
pub use crate::chain::get_agent_activity;
pub use crate::chain::query;
pub use crate::ed25519::sign;
pub use crate::ed25519::sign_ephemeral;
pub use crate::ed25519::sign_ephemeral_raw;
pub use crate::ed25519::sign_raw;
pub use crate::ed25519::verify_signature;
pub use crate::ed25519::verify_signature_raw;
pub use crate::entry::create;
pub use crate::entry::create_entry;
pub use crate::entry::delete;
pub use crate::entry::delete_entry;
pub use crate::entry::get;
pub use crate::entry::get_details;
pub use crate::entry::hash_entry;
pub use crate::entry::must_get_entry;
pub use crate::entry::must_get_header;
pub use crate::entry::must_get_valid_element;
pub use crate::entry::update;
pub use crate::entry::update_entry;
pub use crate::entry::EntryDefRegistration;
pub use crate::entry_def;
pub use crate::entry_def_index;
pub use crate::entry_defs;
pub use crate::entry_type;
pub use crate::hash_path::anchor::anchor;
pub use crate::hash_path::anchor::get_anchor;
pub use crate::hash_path::anchor::list_anchor_addresses;
pub use crate::hash_path::anchor::list_anchor_tags;
pub use crate::hash_path::anchor::list_anchor_type_addresses;
pub use crate::hash_path::anchor::Anchor;
pub use crate::hash_path::path::Path;
pub use crate::hdk::*;
pub use crate::info::agent_info;
pub use crate::info::zome_info;
pub use crate::link::create_link;
pub use crate::link::delete_link;
pub use crate::link::get_link_details;
pub use crate::link::get_links;
pub use crate::map_extern;
pub use crate::map_extern::ExternResult;
pub use crate::p2p::call;
pub use crate::p2p::call_remote;
pub use crate::p2p::emit_signal;
pub use crate::p2p::remote_signal;
pub use crate::random::*;
pub use crate::register_entry;
pub use crate::time::schedule;
pub use crate::time::sleep;
pub use crate::time::sys_time;
pub use crate::time::*;
pub use crate::x_salsa20_poly1305::create_x25519_keypair;
pub use crate::x_salsa20_poly1305::x_25519_x_salsa20_poly1305_decrypt;
pub use crate::x_salsa20_poly1305::x_25519_x_salsa20_poly1305_encrypt;
pub use crate::x_salsa20_poly1305::x_salsa20_poly1305_decrypt;
pub use crate::x_salsa20_poly1305::x_salsa20_poly1305_encrypt;
pub use hdk_derive::hdk_entry;
pub use hdk_derive::hdk_extern;
pub use holo_hash;
pub use holo_hash::AgentPubKey;
pub use holo_hash::AnyDhtHash;
pub use holo_hash::EntryHash;
pub use holo_hash::EntryHashes;
pub use holo_hash::HasHash;
pub use holo_hash::HeaderHash;
pub use holo_hash::HoloHash;
pub use holo_hash::HoloHashed;
pub use holochain_wasmer_guest::*;
pub use holochain_zome_types;
pub use holochain_zome_types::prelude::*;
pub use std::collections::BTreeSet;
pub use std::collections::HashSet;
pub use std::convert::TryFrom;
pub use tracing;
pub use tracing::{debug, error, info, instrument, trace, warn};
pub use tracing_subscriber;

#[cfg(feature = "mock")]
pub use mockall;

// This needs to be called at least once _somewhere_ and is idempotent.
#[macro_export]
macro_rules! holochain_externs {
    () => {
        holochain_wasmer_guest::host_externs!(
            __trace,
            __hash_entry,
            __unreachable,
            __verify_signature,
            __sign,
            __sign_ephemeral,
            __zome_info,
            __call_info,
            __dna_info,
            __app_info,
            __random_bytes,
            __sys_time,
            __agent_info,
            __capability_claims,
            __capability_grants,
            __capability_info,
            __get,
            __get_details,
            __get_links,
            __get_link_details,
            __get_agent_activity,
            __must_get_entry,
            __must_get_valid_element,
            __must_get_header,
            __query,
            __call_remote,
            __call,
            __create,
            __emit_signal,
            __remote_signal,
            __create_link,
            __delete_link,
            __update,
            __delete,
            __schedule,
            __sleep,
            __x_salsa20_poly1305_encrypt,
            __x_salsa20_poly1305_decrypt,
            __x_25519_x_salsa20_poly1305_encrypt,
            __x_25519_x_salsa20_poly1305_decrypt,
            __create_x25519_keypair
        );
    };
}

#[cfg(not(feature = "mock"))]
holochain_externs!();
