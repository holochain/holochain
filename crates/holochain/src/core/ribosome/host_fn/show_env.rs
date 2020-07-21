use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::ShowEnvInput;
use holochain_zome_types::ShowEnvOutput;
use std::sync::Arc;

pub fn show_env(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<CallContext>,
    _input: ShowEnvInput,
) -> RibosomeResult<ShowEnvOutput> {
    unimplemented!();
}
