use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::RemoveLinkInput;
use holochain_zome_types::RemoveLinkOutput;
use std::sync::Arc;

pub fn remove_link(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<CallContext>,
    _input: RemoveLinkInput,
) -> RibosomeResult<RemoveLinkOutput> {
    unimplemented!();
}
