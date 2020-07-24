use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::ScheduleInput;
use holochain_zome_types::ScheduleOutput;
use std::sync::Arc;

pub fn schedule(
    _ribosome: Arc<WasmRibosome>,
    _call_context: Arc<CallContext>,
    _input: ScheduleInput,
) -> RibosomeResult<ScheduleOutput> {
    unimplemented!()
}
