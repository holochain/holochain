//! Some common testing helpers.

use crate::{cell::CellId, prelude::*};
use std::collections::BTreeMap;
use sx_zome_types::agent::AgentId;
use sx_zome_types::dna::{
    bridges::Bridge,
    capabilities::CapabilityRequest,
    entry_types::EntryTypeDef,
    fn_declarations::{FnDeclaration, TraitFns},
    wasm::DnaWasm,
    zome::{Config, Zome, ZomeFnDeclarations},
    Dna,
};
use sx_zome_types::signature::{Provenance, Signature};
use sx_zome_types::prelude::*;


/// A fixture example CellId for unit testing.
pub fn fake_cell_id(name: &str) -> CellId {
    (name.to_string().into(), fake_agent_id(name)).into()
}

/// A fixture example CapabilityRequest for unit testing.
pub fn fake_capability_request() -> CapabilityRequest {
    CapabilityRequest {
        cap_token: Address::from("fake"),
        provenance: fake_provenance(),
    }
}

/// A fixture example ZomeInvocationPayload for unit testing.
pub fn fake_zome_invocation_payload() -> ZomeExternHostInput {
    ZomeExternHostInput::try_from(SerializedBytes::try_from(()).unwrap()).unwrap()
}

/// A fixture example Signature for unit testing.
pub fn fake_signature() -> Signature {
    Signature::from("fake")
}

/// A fixture example Provenance for unit testing.
pub fn fake_provenance() -> Provenance {
    Provenance::new("fake".into(), fake_signature())
}
