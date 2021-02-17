pub use crate::app_entry;
pub use crate::capability::create_cap_claim::create_cap_claim;
pub use crate::capability::create_cap_grant::create_cap_grant;
pub use crate::capability::delete_cap_grant::delete_cap_grant;
pub use crate::capability::generate_cap_secret::generate_cap_secret;
pub use crate::capability::update_cap_grant::update_cap_grant;
pub use crate::entry::create_entry::create_entry;
pub use crate::entry::delete_entry::delete_entry;
pub use crate::entry::hash_entry::hash_entry;
pub use crate::entry::update_entry::update_entry;
pub use crate::entry_def;
pub use crate::entry_def_index;
pub use crate::entry_defs;
pub use crate::entry_interface;
pub use crate::guest_callback::entry_defs::EntryDefRegistration;
pub use crate::hash_path::anchor::anchor;
pub use crate::hash_path::anchor::get_anchor;
pub use crate::hash_path::anchor::list_anchor_addresses;
pub use crate::hash_path::anchor::list_anchor_tags;
pub use crate::hash_path::anchor::list_anchor_type_addresses;
pub use crate::hash_path::anchor::Anchor;
pub use crate::hash_path::path::Path;
pub use crate::host_fn::agent_info::agent_info;
pub use crate::host_fn::call::call;
pub use crate::host_fn::call_remote::call_remote;
pub use crate::host_fn::create::create;
pub use crate::host_fn::create_link::create_link;
pub use crate::host_fn::delete::delete;
pub use crate::host_fn::delete_link::delete_link;
pub use crate::host_fn::emit_signal::emit_signal;
pub use crate::host_fn::get::get;
pub use crate::host_fn::get_agent_activity::get_agent_activity;
pub use crate::host_fn::get_details::get_details;
pub use crate::host_fn::get_link_details::get_link_details;
pub use crate::host_fn::get_links::get_links;
pub use crate::host_fn::query::query;
pub use crate::host_fn::random_bytes::random_bytes;
pub use crate::host_fn::random_bytes::*;
pub use crate::host_fn::remote_signal::remote_signal;
pub use crate::host_fn::sign::sign;
pub use crate::host_fn::sign::sign_raw;
pub use crate::host_fn::sys_time::sys_time;
pub use crate::host_fn::update::update;
pub use crate::host_fn::verify_signature::verify_signature;
pub use crate::host_fn::verify_signature::verify_signature_raw;
pub use crate::host_fn::zome_info::zome_info;
pub use crate::map_extern;
pub use crate::map_extern::ExternResult;
pub use crate::x_salsa20_poly1305::create_x25519_keypair::create_x25519_keypair;
pub use crate::x_salsa20_poly1305::x_25519_x_salsa20_poly1305_decrypt::x_25519_x_salsa20_poly1305_decrypt;
pub use crate::x_salsa20_poly1305::x_25519_x_salsa20_poly1305_encrypt::x_25519_x_salsa20_poly1305_encrypt;
pub use crate::x_salsa20_poly1305::x_salsa20_poly1305_decrypt::x_salsa20_poly1305_decrypt;
pub use crate::x_salsa20_poly1305::x_salsa20_poly1305_encrypt::x_salsa20_poly1305_encrypt;
pub use hdk3_derive::hdk_entry;
pub use hdk3_derive::hdk_extern;
pub use holo_hash::AgentPubKey;
pub use holo_hash::AnyDhtHash;
pub use holo_hash::EntryHash;
pub use holo_hash::EntryHashes;
pub use holo_hash::HasHash;
pub use holo_hash::HeaderHash;
pub use holo_hash::HoloHash;
pub use holochain_wasmer_guest::*;
pub use holochain_zome_types;
pub use holochain_zome_types::prelude::*;
pub use std::collections::HashSet;
pub use std::convert::TryFrom;
pub use tracing;
pub use tracing::{debug, error, info, instrument, trace, warn};
pub use tracing_subscriber;

// This needs to be called at least once _somewhere_ and is idempotent.
#[macro_export]
macro_rules! holochain_externs {
    () => {
        holochain_wasmer_guest::memory_externs!();
        holochain_wasmer_guest::host_externs!(
            __trace,
            __hash_entry,
            __unreachable,
            __verify_signature,
            __sign,
            __decrypt,
            __encrypt,
            __zome_info,
            __property,
            __random_bytes,
            __show_env,
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
            __x_salsa20_poly1305_encrypt,
            __x_salsa20_poly1305_decrypt,
            __x_25519_x_salsa20_poly1305_encrypt,
            __x_25519_x_salsa20_poly1305_decrypt,
            __create_x25519_keypair
        );
    };
}

holochain_externs!();
