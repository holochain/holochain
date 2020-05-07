use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use holochain_zome_types::ScheduleInput;
use holochain_zome_types::ScheduleOutput;
use std::sync::Arc;

pub async fn schedule(
    _ribosome: Arc<WasmRibosome<'_>>,
    _host_context: Arc<HostContext>,
    _input: ScheduleInput,
) -> RibosomeResult<ScheduleOutput> {
    unimplemented!()
}
