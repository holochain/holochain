use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::QueryInput;
use holochain_zome_types::QueryOutput;
use std::sync::Arc;

pub async fn query(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: QueryInput,
) -> RibosomeResult<QueryOutput> {
    unimplemented!();
}
