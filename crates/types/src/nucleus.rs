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
#[allow(missing_docs)] // members are self-explanitory
// DO NOT CLONE THIS because payload can be huge
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocation {
    /// The ID of the [Cell] in which this Zome-call would be invoked
    pub cell_id: CellId,
    /// The name of the Zome containing the function that would be invoked
    pub zome_name: ZomeName,
    /// The capability request authorization this [ZomeInvocation]
    pub cap: CapabilityRequest,
    /// The name of the Zome function to call
    pub fn_name: String,
    /// The serialized data to pass an an argument to the Zome call
    pub payload: ZomeExternHostInput,
    /// the provenance of the call
    pub provenance: AgentId,
    /// the hash of the top header at the time of call
    pub as_at: Address,
}

/// Response to a zome invocation
#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ZomeInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(ZomeExternGuestOutput),
}
