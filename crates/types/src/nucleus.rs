//! Types related to making calls into Zomes.
use crate::cell::CellId;
use holochain_zome_types::zome::ZomeName;

/// The ZomeId is a pair of CellId and ZomeName.
pub type ZomeId = (CellId, ZomeName);

/// ZomeName as a String (should this be a newtype?)
pub type ZomeName = String;

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
// DO NOT CLONE THIS because payload can be huge
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocation {
    /// The ID of the [Cell] in which this Zome-call would be invoked
    pub cell_id: CellId,
    /// The name of the Zome containing the function that would be invoked
    pub zome_name: ZomeName,
    /// The capability request authorization this [ZomeInvocation]
    pub cap: CapToken,
    /// The name of the Zome function to call
    pub fn_name: String,
    /// The serialized data to pass an an argument to the Zome call
    pub payload: ZomeExternHostInput,
    /// the provenance of the call
    pub provenance: AgentPubKey,
    /// the hash of the top header at the time of call
    pub as_at: HeaderHash,
}

/// Response to a zome invocation
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ZomeInvocationResponse {
    /// arbitrary functions exposed by zome devs to the outside world
    ZomeApiFn(ZomeExternGuestOutput),
}
