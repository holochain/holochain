//! Common types

pub use crate::action::*;
pub use crate::agent_activity::*;
pub use crate::block::*;
pub use crate::bytes::*;
pub use crate::call::*;
pub use crate::capability::*;
pub use crate::cell::*;
pub use crate::chain::*;
pub use crate::clone::*;
pub use crate::countersigning::*;
pub use crate::crdt::*;
pub use crate::dna_def::*;
pub use crate::entry::*;
pub use crate::entry_def::*;
#[cfg(feature = "fixturators")]
pub use crate::fixt::*;
pub use crate::genesis::*;
pub use crate::info::*;
pub use crate::init::*;
pub use crate::judged::*;
pub use crate::link::*;
pub use crate::metadata::*;
pub use crate::op::*;
#[cfg(feature = "properties")]
pub use crate::properties::*;
pub use crate::query::ChainQueryFilter as QueryFilter;
pub use crate::query::*;
pub use crate::request::*;
pub use crate::schedule::*;
pub use crate::signal::*;
pub use crate::signature::*;
#[cfg(feature = "test_utils")]
pub use crate::test_utils::*;
pub use crate::validate::*;
pub use crate::warrant::*;
pub use crate::x_salsa20_poly1305::*;
#[cfg(feature = "full-dna-def")]
pub use crate::zome::inline_zome::*;
pub use crate::zome::*;
pub use crate::zome_io::ExternIO;
pub use crate::zome_io::*;
pub use holochain_integrity_types::prelude::*;

// `capability::CapAccess` (data-carrying grant access) and `action::CapAccess`
// (the `CapGrant.cap_access` column discriminant) share a name; an explicit
// re-export takes priority over the glob re-exports above, resolving the
// ambiguity in favor of the grant-access type at the crate root.
// `action::CapAccess` remains reachable via its `action::` path.
pub use crate::capability::CapAccess;

// `crate::action::SignedActionHashed` and
// `holochain_integrity_types::action::SignedActionHashed` are the same
// underlying alias (`SignedHashed<Action>`) defined in two crates; an
// explicit re-export takes priority over the glob re-exports above,
// resolving the ambiguity in favor of this crate's own alias, which is the
// path downstream consumers use.
pub use crate::action::SignedActionHashed;

// Bring the validation `Op` and its variant structs into the prelude so
// validators and inline zomes decode the `Op` the host encodes. `AgentActivity`
// (the `Op` variant struct, one action + optionally its cached entry) and
// `query::AgentActivity` (the richer `get_agent_activity` response type) share
// a name; both are glob re-exported (this block and `crate::query::*` below),
// so an explicit re-export takes priority, resolving the ambiguity in favor of
// the `Op` variant struct — the path the large majority of existing consumers
// use (e.g. every per-action item built while scanning a chain). Consumers
// that need the richer query-response type instead import
// `crate::query::AgentActivity` explicitly.
pub use crate::op::{
    AgentActivity, CreateEntry, CreateLink, CreateRecord, Delete, DeleteLink, Op, Update,
};
