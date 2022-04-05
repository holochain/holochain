use crate::prelude::*;

/// Get the DNA information.
/// There are no inputs to [ `dna_info` ].
///
/// DNA information includes dna name, hash, properties, and zome names.
pub fn dna_info() -> ExternResult<DnaInfo> {
    IDK.with(|h| h.borrow().dna_info(()))
}

/// Get the zome information.
/// There are no inputs to [ `zome_info` ].
///
/// Zome information includes zome name, id and properties.
///
/// In general any holochain compatible wasm can be compiled and run in any zome so the zome info
/// needs to be looked up at runtime to e.g. know where to send/receive `call_remote` rpc calls to.
pub fn zome_info() -> ExternResult<ZomeInfo> {
    IDK.with(|h| h.borrow().zome_info(()))
}
