//! Types related to making calls into Zomes.

use crate::{agent::AgentId, cell::CellId, dna::capabilities::CapabilityRequest, prelude::*};
// use std::sync::Arc;
use sx_zome_types::*;

/// The ZomeId is a pair of CellId and ZomeName.
pub type ZomeId = (CellId, ZomeName);

/// ZomeName as a String (should this be a newtype?)
pub type ZomeName = String;

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[derive(Debug)]
pub struct ZomeInvocation {
    /// the cell ID
    pub cell_id: CellId,
    /// the zome name
    pub zome_name: ZomeName,
    /// a capability request
    pub cap: CapabilityRequest,
    /// the zome fn to call
    pub fn_name: String,
    /// the serialized data to make available to the zome call
    pub payload: ZomeExternHostInput,
    /// the provenance of the call
    pub provenance: AgentId,
    /// the hash of the top header at the time of call
    pub as_at: Address,
}

/// Is this a stub??
#[derive(Debug, PartialEq)]
pub enum ZomeInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(ZomeExternGuestOutput),
}
