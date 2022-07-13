pub use crate::app_entry;
pub use crate::ed25519::verify_signature;
pub use crate::ed25519::verify_signature_raw;
pub use crate::entry::must_get_action;
pub use crate::entry::must_get_entry;
pub use crate::entry::must_get_valid_record;
pub use crate::entry_defs;
pub use crate::hash::*;
pub use crate::hdi::*;
pub use crate::info::dna_info;
pub use crate::info::zome_info;
pub use crate::link::LinkTypeFilterExt;
pub use crate::map_extern;
pub use crate::map_extern::ExternResult;
pub use crate::map_extern_infallible;
pub use crate::map_extern_preamble;
pub use crate::op::*;
pub use crate::x_salsa20_poly1305::x_25519_x_salsa20_poly1305_decrypt;
pub use crate::x_salsa20_poly1305::x_salsa20_poly1305_decrypt;
pub use hdk_derive;
pub use hdk_derive::hdk_entry_defs;
pub use hdk_derive::hdk_entry_defs_conversions;
pub use hdk_derive::hdk_entry_helper;
pub use hdk_derive::hdk_extern;
pub use hdk_derive::hdk_link_types;
pub use hdk_derive::hdk_to_coordinates;
pub use hdk_derive::EntryDefRegistration;
pub use hdk_derive::UnitEnum;
pub use holo_hash;
pub use holo_hash::ActionHash;
pub use holo_hash::AgentPubKey;
pub use holo_hash::AnyDhtHash;
pub use holo_hash::AnyLinkableHash;
pub use holo_hash::EntryHash;
pub use holo_hash::EntryHashes;
pub use holo_hash::ExternalHash;
pub use holo_hash::HasHash;
pub use holo_hash::HoloHash;
pub use holo_hash::HoloHashed;
pub use holochain_integrity_types;
pub use holochain_integrity_types::prelude::*;
pub use holochain_wasmer_guest::*;
pub use std::collections::BTreeSet;
pub use std::collections::HashSet;
pub use std::convert::TryFrom;
#[cfg(feature = "trace")]
pub use tracing;
#[cfg(feature = "trace")]
pub use tracing::{debug, error, info, instrument, trace, warn};

#[cfg(not(feature = "trace"))]
/// Needed as a noop for map_extern! when trace is off.
pub use crate::error;

#[doc(hidden)]
#[cfg(not(feature = "trace"))]
#[macro_export]
/// Needed as a noop for map_extern! when trace is off.
macro_rules! error {
    ($($field:tt)*) => {};
}

#[cfg(feature = "mock")]
pub use mockall;

// This needs to be called at least once _somewhere_ and is idempotent.
#[doc(hidden)]
#[macro_export]
macro_rules! holochain_externs {
    () => {
        holochain_wasmer_guest::host_externs!(
            __trace,
            __hash,
            __unreachable,
            __verify_signature,
            __zome_info,
            __dna_info,
            __must_get_entry,
            __must_get_valid_record,
            __must_get_action,
            __x_salsa20_poly1305_decrypt,
            __x_25519_x_salsa20_poly1305_decrypt
        );
    };
}

#[cfg(not(feature = "mock"))]
holochain_externs!();
