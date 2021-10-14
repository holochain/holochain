use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::DnaInfo;
use crate::core::ribosome::HostFnAccess;
use holo_hash::HasHash;
use holochain_types::prelude::*;

pub fn dna_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<DnaInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings_deterministic: Permission::Allow, .. } => {
            Ok(DnaInfo {
                name: ribosome.dna_def().name.clone(),
                hash: ribosome.dna_def().as_hash().clone(),
                properties: ribosome.dna_def().properties.clone()
            })
        },
        _ => unreachable!(),
    }
}

