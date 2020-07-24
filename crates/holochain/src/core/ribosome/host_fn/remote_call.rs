use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::RemoteCallInput;
use holochain_zome_types::RemoteCallOutput;
use std::sync::Arc;

pub fn remote_call(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<CallContext>,
    _input: RemoteCallInput,
) -> RibosomeResult<RemoteCallOutput> {
    unimplemented!();
}
